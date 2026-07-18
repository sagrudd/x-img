Segmented video capability gate
===============================

HLS, DASH, and Media Source Extensions are not progressive MP4 files. Their
manifests, segments, encryption, and changing signed URLs require bounded
identity evidence. Pinakotheke provides a site-neutral *planning* adapter for
media Firefox already observed while the user played it. The planner does not
fetch a manifest, enumerate a playlist, inspect hidden resources, or rewrite
playback.

The ``x-img.segmented-media-plan.v1`` request contains an exact enabled
HTTPS origin, adapter and canonicalization versions, a canonical manifest or
MSE-presentation SHA-256, and at most 256 ordered segment identity SHA-256
values. It accepts at most 16 short codec/container family diagnostics and 16
GiB of declared media. It contains no media URL, signed query, cookie,
authorization header, browser credential, page title, or browsing history.

Planning requirements
---------------------

Planning succeeds only when:

* Firefox recorded a user play on the enabled origin;
* every identity came from a resource already observed during that play;
* no hidden traversal or playlist/channel crawl occurred;
* policy permits capture and no authorization context was observed;
* the presentation is clear, without encryption or DRM; and
* the exact adapter proof covers identity, deterministic retry/idempotency,
  policy and DRM blocking, and fail-open origin behavior.

The plan ID is derived from the adapter version, manifest identity, and ordered
segment identities. Repeating the same observation therefore returns exactly
the same plan. Duplicate, unordered, empty, oversized, or malformed evidence
is blocked. A block never changes the video element: normal origin playback
continues.

Required adapter evidence
-------------------------

After a planned source has been normalized, the separate server-side
``x-img.segmented-video-gate.v1`` substitution decision still requires:

* an exact HTTPS origin, adapter ID, and semantic adapter version;
* an explicit HLS or DASH manifest kind;
* separate semantic versions for manifest and segment canonicalization;
* a synthetic, redistributable fixture evidence ID;
* real Firefox evidence for the exact adapter behavior;
* an explicitly opened, user-displayed candidate;
* no DRM or encryption; and
* a ``Ready`` normalized Pinakotheke playback profile matching the evidence.

Missing, malformed, stale, or mismatched evidence produces ``Origin served``.
It never selects another adapter, endpoint, ObjectStore, source rendition, or
credential path. Planning does not itself authorize acquisition or fetch
media; substitution requires a separately verified normalized rendition.

Firefox behavior
----------------

On an enabled video site, only a user play may begin observation. The extension
may submit canonical identity hashes for network resources the browser already
observed during that play; it may not fetch a playlist, follow unobserved
references, read credentials, or retain raw signed URLs. Blob/MSE sources use a
presentation identity and the same bounds. Until normalization and playback
proof are ready, settings report a bounded ``Origin served`` diagnostic and do
not change the video element.

Progressive normalized MP4 remains supported through :doc:`mp4-substitution`.
DRM/encrypted, unobserved, unopened, source-only, failed, or unverified media
always stays with the origin.

Verification
------------

.. code-block:: console

   cargo +1.97.0 test -p pinakotheke-core segmented_video
   node --check firefox-extension/background.js
   node --check firefox-extension/options.js
   docker build --pull --progress=plain -f docs/Dockerfile -t x-img-docs:check .
   docker run --rm x-img-docs:check

The redistributable fixture matrix is
``fixtures/segmented-media/v1/cases.json``. It proves HLS planning and retry,
DASH policy blocking, MSE DRM blocking, and authorization-context rejection
without real sites, URLs, credentials, or payload bytes.

Compatibility-sensitive contracts were inspected at DASObjectStore
``8afcfb487120f5fa9d0431b3ae8ce0fc4a42af37``, Monas
``799484eeb1f6d324500f8ed59bed8e43deed7be5``, Mnemosyne design language
``fbfa28e55d1c8111ef95a139d83927c231534b5f``, and future Synoptikon
``52810176bf95a170f93d74a6f5daa94da5c6640e``.  No unpublished path dependency
is used.
