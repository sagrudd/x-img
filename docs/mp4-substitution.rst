Normalized MP4 substitution
===========================

x-img can replace a visible, explicitly opened progressive video with its
reviewed normalized MP4 rendition.  This applies only to an HTTPS site whose
user-enabled rule includes videos and substitution, and whose versioned
adapter declares ``mp4_substitution``.  HLS, DASH, encrypted media, source-only
objects, and unverified renditions remain origin-served.

Delivery and authority
----------------------

Alias lookup returns a video-specific opaque route only for
``NormalizedMp4`` with ``ExplicitlyOpenedOriginal`` evidence:

.. code-block:: text

   GET /api/cache/v1/videos/{pairing_id}/{delivery_id}

The route repeats actor, pairing, instance, origin, adapter/version, policy,
expiry, availability, and representation checks before every read.  It then
streams the exact reviewed endpoint/ObjectStore/object through the authorized
DASObjectStore read port.  It accepts one byte range, returns ``206`` with
exact ``Content-Range``, rejects invalid or multiple ranges with ``416``, and
supports checksum ETag conditional ``304`` responses.  Full and partial
responses preserve ``video/mp4``, length, ETag, ``Accept-Ranges: bytes``, exact
origin CORS with credentials, exposed range headers, CORP, nosniff, and
private/no-store policy.  Concurrent browser streams share authority metadata
but retain independent response bodies.

Firefox behavior
----------------

A toolbar action considers at most eight visible videos that already loaded
source metadata.  The extension strips query and fragment data for lookup and
sets the cache route directly on the matching native ``video`` element with
``crossorigin=use-credentials``.  Firefox therefore owns range scheduling,
seeking, pause/resume, cancellation, and decoding; video is never buffered into
extension memory or browser storage.

The extension retains the original ``src``, child ``source`` attributes,
cross-origin mode, time, and playing state only in the injected page call.  A
timeout, TLS/CSP/CORS/CORP/session/range/MIME/decode error restores the origin
once and calls ``load`` without retrying x-img or choosing another store.

Verification
------------

Rust tests cover authorization, normalized-only eligibility, concurrent
ranges, ``206``, ``304``, and ``416``.  The real-Firefox harness uses a
Docker-generated, ephemeral H.264/AAC MP4 and proves range playback,
concurrent ranges, conditional requests, cancellation, seek, pause/resume, and
origin fallback.  The video and Firefox profile are deleted after the run.

Compatibility-sensitive contracts were inspected at DASObjectStore
``42bf66a7494f4e0aa81f103100b71489b38164dc``, Monas
``3d21b0bc7b83fa8408d01b93347a56f43f3a96b7``, Mnemosyne design language
``5539df8f662a78ebdf7cf4c868d71831380c8cfd``, and future Synoptikon
``52810176bf95a170f93d74a6f5daa94da5c6640e``.  The public build has no sibling
path dependency.

.. code-block:: console

   cargo +1.97.0 test -p pinakotheke-core cache_alias
   cargo +1.97.0 test -p pinakotheke-api cached_video
   python3 scripts/firefox/check_normalized_playback.py --video /ephemeral/normalized.mp4
   docker build --pull --progress=plain -f docs/Dockerfile -t x-img-docs:check .
   docker run --rm x-img-docs:check
