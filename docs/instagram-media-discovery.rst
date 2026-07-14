Incremental Instagram media discovery
=====================================

XIMG-044 defines a fixture-driven metadata planner for a future approved
Instagram connector. It does not contact Meta or Instagram, accept a token,
copy browser credentials, download media, or write bytes. Until XIMG-043 closes
the approved product, account-class, permission, app-review, rights, retention,
and deletion gates, it remains fixture-only under :doc:`adr/0002-platform-policy`.

Supported fixture records
-------------------------

The planner accepts versioned synthetic pages for one permitted account. A page
has a request and next cursor and contains one-media posts, multi-media
carousels, or one-media reels. Image entries accept JPEG, PNG, and WebP;
video entries accept MP4. The largest supported rendition is selected, so an
HLS-only or unsupported rendition fails closed rather than creating a fallback
browser download path.

Pagination stops at the explicit page or candidate limit and returns both the
last next cursor and a ``truncated_by_budget`` word-state. A cursor that does
not match the preceding page is rejected. A carousel must have at least two
media entries; a post or reel must have exactly one. These checks make an
unsupported item shape explicit rather than guessing its API meaning.

Opaque token lifecycle
----------------------

Only a non-secret host-token state enters this boundary. ``FixtureActive`` is
synthetic test evidence, not a credential or live authorization. ``Expired``
and ``Revoked`` yield ``ReauthorizationRequired`` before pages are planned.
No raw access token, refresh token, browser cookie, password, or authorization
header is accepted, stored, logged, or included in the fixture.

Provenance and idempotency
--------------------------

Each candidate retains the Instagram account, item and media IDs; item/media
kind; canonical ``instagram:<account>:<item>:<media>`` identity; safe source
URL alias; selected rendition; expected checksum; discovery time; adapter
version; and policy result. The candidate can create the existing
``ReconciliationRequest`` keyed by canonical identity plus checksum. Duplicate
page entries therefore converge to one verified metadata settlement once an
authorized future DASObjectStore ingest supplies object evidence.

No discovery result is a media transfer or review admission. Verified storage,
policy revalidation, deletion handling, and acquisition lifecycle settlement
remain required before any future item can be shown as new or reviewed.

Verify locally
--------------

.. code-block:: console

   cargo +1.97.0 test -p x-img-core instagram_discovery
   docker build --pull --progress=plain -f docs/Dockerfile -t x-img-docs:check .
   docker run --rm x-img-docs:check
