// SPDX-License-Identifier: MPL-2.0
// No page cookies, webRequest interception, credentials, or automatic navigation.
browser.runtime.onInstalled.addListener(() =>
  browser.storage.local.set({ instanceUrl: "", instanceId: "", pairId: "", sites: [] }),
);

async function matchingAdapter(url) {
  const registry = await fetch(browser.runtime.getURL("adapters.json")).then(response => response.json());
  const target = new URL(url);
  return registry.adapters.find(adapter =>
    adapter.origins.includes(target.origin)
      && !adapter.exclude_paths.some(path => target.pathname.startsWith(path))
      && (adapter.capabilities.observed_thumbnail || adapter.capabilities.image_substitution),
  ) || null;
}

function canonicalAlias(rawUrl) {
  const url = new URL(rawUrl);
  if (url.protocol !== "https:") throw new Error("only HTTPS aliases are eligible");
  url.search = "";
  url.hash = "";
  return url.href;
}

async function substituteDisplayedImage(candidate) {
  const canonical = rawUrl => {
    const url = new URL(rawUrl);
    url.search = "";
    url.hash = "";
    return url.href;
  };
  const image = [...document.images].find(item => {
    const rect = item.getBoundingClientRect();
    return item.currentSrc
      && canonical(item.currentSrc) === candidate.canonicalAlias
      && rect.width > 0 && rect.height > 0
      && rect.bottom > 0 && rect.right > 0
      && rect.top < window.innerHeight && rect.left < window.innerWidth;
  });
  if (!image) return { outcome: "origin", reason: "image_not_visible" };

  const originalSrc = image.getAttribute("src");
  const originalSrcset = image.getAttribute("srcset");
  let blobUrl;
  const restore = () => {
    if (originalSrc === null) image.removeAttribute("src"); else image.setAttribute("src", originalSrc);
    if (originalSrcset === null) image.removeAttribute("srcset"); else image.setAttribute("srcset", originalSrcset);
    if (blobUrl) URL.revokeObjectURL(blobUrl);
  };
  try {
    const controller = new AbortController();
    const timer = setTimeout(() => controller.abort(), 5000);
    let response;
    try {
      response = await fetch(candidate.deliveryUrl, {
        credentials: "include",
        cache: "no-store",
        redirect: "error",
        signal: controller.signal,
      });
    } finally {
      clearTimeout(timer);
    }
    const contentType = response.headers.get("content-type");
    const contentLength = Number(response.headers.get("content-length"));
    const etag = response.headers.get("etag");
    if (!response.ok
      || contentType !== candidate.contentType
      || contentLength !== candidate.contentLength
      || contentLength < 1 || contentLength > 32 * 1024 * 1024
      || etag !== `\"${candidate.objectChecksum}\"`) {
      throw new Error("delivery metadata mismatch");
    }
    const bytes = await response.arrayBuffer();
    if (bytes.byteLength !== contentLength) throw new Error("delivery length mismatch");
    blobUrl = URL.createObjectURL(new Blob([bytes], { type: contentType }));
    image.removeAttribute("srcset");
    await new Promise((resolve, reject) => {
      const loaded = () => { cleanup(); resolve(); };
      const failed = () => { cleanup(); reject(new Error("replacement rejected")); };
      const cleanup = () => {
        image.removeEventListener("load", loaded);
        image.removeEventListener("error", failed);
      };
      image.addEventListener("load", loaded, { once: true });
      image.addEventListener("error", failed, { once: true });
      image.setAttribute("src", blobUrl);
    });
    URL.revokeObjectURL(blobUrl);
    blobUrl = undefined;
    return { outcome: "objectstore" };
  } catch (_) {
    restore();
    return { outcome: "origin", reason: "delivery_failed" };
  }
}

function displayedImages() {
  return [...document.images]
    .filter(image => {
      const style = window.getComputedStyle(image);
      const rect = image.getBoundingClientRect();
      return image.complete
        && image.currentSrc
        && image.naturalWidth > 0
        && image.naturalHeight > 0
        && style.display !== "none"
        && style.visibility !== "hidden"
        && Number(style.opacity) > 0
        && rect.width > 0
        && rect.height > 0
        && rect.bottom > 0
        && rect.right > 0
        && rect.top < window.innerHeight
        && rect.left < window.innerWidth;
    })
    .map(image => ({ url: image.currentSrc, width: image.naturalWidth, height: image.naturalHeight }))
    .slice(0, 32);
}

browser.action.onClicked.addListener(async tab => {
  try {
    if (!tab.id || !tab.url) return;
    const origin = new URL(tab.url).origin;
    const { instanceUrl, instanceId, pairId, sites = [] } = await browser.storage.local.get(["instanceUrl", "instanceId", "pairId", "sites"]);
    const rule = sites.find(site => site.origin === origin);
    if (!rule?.media.includes("images") || (!rule.capture && !rule.substitution)
      || !instanceUrl || !instanceId || !pairId) return;
    const adapter = await matchingAdapter(tab.url);
    if (!adapter) return;
    const [{ result = [] }] = await browser.scripting.executeScript({
      target: { tabId: tab.id },
      func: displayedImages,
    });
    for (const observed of result) {
      if (rule.capture && adapter.capabilities.observed_thumbnail) {
        await fetch(`${instanceUrl}/api/extension/v1/capture-plans`, {
          method: "POST",
          credentials: "include",
          headers: { "content-type": "application/json" },
          body: JSON.stringify({
            schema_version: "x-img.capture-request.v1",
            pairing_id: pairId,
            origin,
            page_url: tab.url,
            adapter_kind: adapter.kind,
            adapter_version: adapter.version,
            capture_kind: "observed_thumbnail",
            media_url: observed.url,
            width: observed.width,
            height: observed.height,
          }),
        });
      }
      if (rule.substitution && adapter.capabilities.image_substitution) {
        const alias = canonicalAlias(observed.url);
        const response = await fetch(`${instanceUrl}/api/extension/v1/cache-aliases/lookup`, {
          method: "POST",
          credentials: "include",
          cache: "no-store",
          headers: { "content-type": "application/json" },
          body: JSON.stringify({
            schema_version: "x-img.cache-alias-lookup.v1",
            pairing_id: pairId,
            instance_id: instanceId,
            origin,
            canonical_alias: alias,
            adapter_id: adapter.id,
            adapter_version: adapter.version,
          }),
        });
        if (!response.ok) continue;
        const hit = await response.json();
        if (hit.schema_version !== "x-img.cache-alias-result.v1" || hit.outcome !== "hit"
          || !hit.media_class?.endsWith("_image") || !hit.delivery_path) continue;
        const deliveryUrl = new URL(hit.delivery_path, instanceUrl);
        if (deliveryUrl.origin !== instanceUrl) continue;
        await browser.scripting.executeScript({
          target: { tabId: tab.id },
          func: substituteDisplayedImage,
          args: [{
            canonicalAlias: alias,
            deliveryUrl: deliveryUrl.href,
            contentType: hit.content_type,
            contentLength: hit.content_length,
            objectChecksum: hit.object_checksum,
          }],
        });
      }
    }
  } catch (_) {
    // Capture is deliberately fail-open: page browsing never depends on x-img.
  }
});
