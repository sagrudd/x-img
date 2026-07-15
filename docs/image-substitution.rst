Image substitution
==================

The Firefox extension can replace a displayed image with the exact reviewed
DASObjectStore object resolved by x-img.  This is an external-cache operation,
not navigation or crawling: it runs only after a toolbar action, only for an
image currently visible in the viewport, and only when substitution is enabled
for that exact HTTPS site and its registered adapter.

Pairing and lookup
------------------

Pairing records the HTTPS x-img origin, stable instance identifier, and opaque
pairing reference.  It stores no site password, page cookie, DAS credential,
or host session.  A displayed source URL is reduced to an HTTPS alias without
its query or fragment before the host-authenticated lookup described in
:doc:`cache-alias-lookup`.

A hit returns an opaque delivery path bound to one reviewed alias record.  The
identifier is not a checksum: identical bytes may legitimately exist in more
than one ObjectStore, so delivery retains the exact endpoint, ObjectStore, and
object key selected during review.  Immediately before reading, the server
revalidates actor, pairing, expiry/revocation, instance, origin, adapter/version,
substitution policy, availability, validity, and image representation.

Delivery contract
-----------------

The authenticated route is:

.. code-block:: text

   GET /api/cache/v1/images/{pairing_id}/{delivery_id}

The route streams the object through the scoped DASObjectStore read port.  It
requires an exact image MIME type, positive full length, SHA-256-bound quoted
ETag, and immutable object reference.  Its browser response uses the reviewed
site as the exact ``Access-Control-Allow-Origin``, permits the existing host
session, exposes only ETag and content length, uses
``Cross-Origin-Resource-Policy: cross-origin``, ``X-Content-Type-Options:
nosniff``, ``Vary: origin``, and ``Cache-Control: private, no-store``.  x-img
does not redirect to the source and does not persist response bytes.

Firefox replacement and fail-open behavior
-------------------------------------------

The page fetch is bounded to five seconds and 32 MiB.  Firefox must receive the
expected status, MIME type, content length, and quoted checksum ETag; the actual
byte count must also match.  Only then does the extension create an ephemeral
blob URL and replace the matching visible image.  The blob URL is revoked after
load.

Any lookup, TLS, session, CSP, CORS, CORP, network, timeout, metadata, size, or
decode failure restores the original ``src`` and ``srcset`` once.  It does not
retry, redirect, loop, change another image, or interfere with ordinary origin
loading.  Stored media bytes are never modified or watermarked.

Verification
------------

The API tests prove authenticated delivery, exact object identity, MIME,
length, ETag, CORS exposure, CORP, and no-store headers.  The local harness uses
an installed Firefox with two isolated loopback origins and runtime-only SVG
bytes to prove successful blob display plus CSP, CORS, MIME, length, and ETag
fail-open.  Its successful response also carries the production CORP header.
The browser profile and image bytes are discarded after the run; production
pairing remains HTTPS-only.

.. code-block:: console

   cargo +1.97.0 test -p x-img-core cache_alias
   cargo +1.97.0 test -p x-img-api cached_image_delivery
   scripts/firefox/check_image_substitution.py
   docker build --pull --progress=plain -f docs/Dockerfile -t x-img-docs:check .
   docker run --rm x-img-docs:check
