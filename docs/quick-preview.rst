Quick preview and normalized video playback
===========================================

The library opens a selected media card in a focused preview task pane.  The
preview is a reading and playback surface, not an acquisition control: it never
opens an origin page, fetches a source URL, stores media bytes, or exposes a
credential.

Using the preview
-----------------

Each preview names the selected record and keeps the following evidence visible:

* source record, media type, ObjectStore state, and descriptive alt text;
* a ``Fit to pane`` / ``View original size`` control for the visual area;
* a source-metadata link, rather than a hidden source navigation; and
* an explicit ``Object unavailable`` state when an authorized object cannot be
  read.  That state never falls back to the source URL.

The current public Web shell uses synthetic visual proxies because x-img never
places image or video payload fixtures in its repository or browser storage.
When the host catalogue provides an authorized ObjectStore reference, the same
layout renders the record metadata and state.

Normalized video
----------------

Only a record marked ``Stored in ObjectStore`` with a verified normalized
playback identifier receives a native HTML video control.  Its URL is the
host-authenticated x-img route:

.. code-block:: text

   /api/playback/v1/{playback_id}

That route preserves the verified MIME type, ETag, and a single byte range;
see :doc:`direct-playback`.  A video without a ready ObjectStore rendition
remains visibly unavailable or non-playable.  The Web client does not supply an
origin URL as a backup.

Keyboard behavior
-----------------

Opening a card moves focus to the preview.  ``Tab`` and ``Shift+Tab`` cycle
through preview actions, ``Escape`` closes the pane, and closing returns focus
to the selected card.  The pane uses words, not colour alone, for object and
delivery state.

Verification
------------

.. code-block:: console

   cargo +1.97.0 test -p x-img-web
   cargo +1.97.0 check -p x-img-web --target wasm32-unknown-unknown
   docker build --pull --progress=plain -f docs/Dockerfile -t x-img-docs:check .
   docker run --rm x-img-docs:check

Compatibility-sensitive sources reviewed: Mnemosyne design language
``5539df8f662a78ebdf7cf4c868d71831380c8cfd``, Monas
``3d21b0bc7b83fa8408d01b93347a56f43f3a96b7``, DASObjectStore
``c44bc513faa5fbcb2a7d6814949e0b4ba29aa480``, and Mnemosyne
``52810176bf95a170f93d74a6f5daa94da5c6640e``.
