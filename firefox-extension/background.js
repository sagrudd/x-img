// SPDX-License-Identifier: MPL-2.0
// No page cookies, webRequest interception, credentials, or automatic navigation.
browser.runtime.onInstalled.addListener(() =>
  browser.storage.local.set({ instanceUrl: "", pairId: "", sites: [] }),
);

async function matchingAdapter(url) {
  const registry = await fetch(browser.runtime.getURL("adapters.json")).then(response => response.json());
  const target = new URL(url);
  return registry.adapters.find(adapter =>
    adapter.origins.includes(target.origin)
      && !adapter.exclude_paths.some(path => target.pathname.startsWith(path))
      && adapter.capabilities.observed_thumbnail,
  ) || null;
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
    const { instanceUrl, pairId, sites = [] } = await browser.storage.local.get(["instanceUrl", "pairId", "sites"]);
    const rule = sites.find(site => site.origin === origin);
    if (!rule?.capture || !rule.media.includes("images") || !instanceUrl || !pairId) return;
    const adapter = await matchingAdapter(tab.url);
    if (!adapter) return;
    const [{ result = [] }] = await browser.scripting.executeScript({
      target: { tabId: tab.id },
      func: displayedImages,
    });
    for (const observed of result) {
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
  } catch (_) {
    // Capture is deliberately fail-open: page browsing never depends on x-img.
  }
});
