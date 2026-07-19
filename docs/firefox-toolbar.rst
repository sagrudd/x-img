Firefox cache toolbar
=====================

The Pinakotheke toolbar popup is the per-site control and diagnostic surface for the
external cache. It operates only on the active tab's exact, explicitly enabled
HTTPS origin. Opening the popup does not scan media or contact x-img; the user
selects ``Run cache for visible media`` to invoke bounded capture/substitution.

The **Video downloads** list reports a bounded set of user-selected videos for
the current origin. Entries move through ``selected``, ``downloading``,
``pending``, and ``stored`` (or ``failed``). ``stored`` is described as
**Available in DASObjectStore** only after authoritative settlement. The list
retains an opaque capture-plan identifier, not a media URL. Autoplay alone
never creates an entry: activation must be associated with the video that
subsequently plays.

The popup identifies itself as ``Pinakotheke cache`` and shows the installed
Firefox extension version read directly from its signed package manifest. This
lets an operator distinguish an older installed extension from a newer server
without exposing browser or server credentials.

Controls
--------

The popup shows ``Active``, ``Paused``, or ``Not enabled`` separately for
capture and substitution. ``Pause substitution`` and ``Resume substitution``
change only the current site's rule; origin browsing continues normally.
``Settings`` opens site policy, while ``Open Pinakotheke source view`` opens the
paired host's Websites context for the already-configured origin.

The permission explanation states that only user-enabled HTTPS origins are in
scope and that cookies, passwords, history, and authorization headers are not
read. Removing a site removes its optional permission and diagnostic entry.

Status and diagnostics
----------------------

Each configured origin has at most one current record. New results replace the
prior result; removed-origin records are deleted. It contains only a worded
state, one coarse reason, and two booleans rendering ``◉ Previously observed``
and ``✓ Stored in ObjectStore``. It contains no page/media URL, alias, signed
query, checksum, object key, cookie, credential, payload, or general history.

The evidence labels use words and iconography rather than colour alone. Their
tooltip explains that this is reversible status framing: stored bytes are never
watermarked or modified. Capture-plan acceptance says ``Previously observed``;
it does not claim ``Stored in ObjectStore`` until reviewed ObjectStore delivery
actually succeeds.

Fail-open behavior
------------------

A miss, host/session failure, unsupported adapter, object outage, or page-side
replacement error becomes ``Origin served``. The toolbar never retries through
another endpoint or ObjectStore and never converts a diagnostic into a page
failure.

Verification
------------

.. code-block:: console

   node --check firefox-extension/background.js
   node --check firefox-extension/popup.js
   node --check firefox-extension/options.js
   python3 scripts/firefox/check_toolbar_contract.py
   docker build --pull --progress=plain -f docs/Dockerfile -t x-img-docs:check .
   docker run --rm x-img-docs:check

Compatibility-sensitive contracts were inspected at DASObjectStore
``d3763d1f3c40d3bedc812c6a733bb54f2bfe64d1``, Monas
``817d956be8ebc3de1956b5e3b4c070098112bc3e``, Mnemosyne design language
``6d654d26c6620381992e89f89e60423d3c02fd86``, and future Synoptikon
``52810176bf95a170f93d74a6f5daa94da5c6640e``. No unpublished path dependency
is used.
