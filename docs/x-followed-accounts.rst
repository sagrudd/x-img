Import followed X accounts
==========================

XIMG-041 defines the review-before-save boundary for adding accounts followed
by the authenticated X viewer. It is a task pane, not a bulk-follow switch:
the person using x-img selects stable X user IDs from the returned permitted
list and sees the resulting configuration diff before saving.

The current implementation is fixture-driven. It makes no X request and does
not enable a live connector while the approval, rights, retention, and deletion
gates in :doc:`adr/0002-platform-policy` remain unresolved.

Task-pane flow
--------------

#. A future official X adapter obtains candidates for the viewing account bound
   to the opaque Monas-held OAuth grant. A protected candidate requires the
   grant to be bound to that same viewing X user ID.
#. The task pane displays each candidate's handle as a label and its stable X
   user ID as the selection key. It requires an explicit selection; unselected
   candidates are never added.
#. ``preview_import`` validates the returned candidates and produces a
   candidate ``InstanceConfig`` plus a word-first review diff: ``Added``,
   ``Already configured``, and ``Not selected``. Duplicate, malformed, or
   invented selections fail closed.
#. A confirmation control calls ``confirm_import``. Without confirmation it
   returns ``Unconfirmed`` and exposes no candidate configuration.
#. After confirmation, the caller passes the returned complete configuration to
   the existing atomic ``ConfigStore::replace`` boundary. The import module
   itself never writes the local JSON allowlist.

The review pane must make the action and its pending state explicit, preserve
keyboard focus, and show failures in words rather than colour alone. Existing
accounts are no-ops in the diff; no account is silently enabled merely because
it appears in the returned follow list.

Authority and privacy
---------------------

The returned list is only an authorized adapter input. It contains no browser
cookies, passwords, access tokens, refresh tokens, or media URLs. The new
allowlist entry carries the same opaque
``monas.connector-authorization`` reference used by strict account
configuration, and applies the configuration's existing media policy, refresh
budget, and review defaults. The X OAuth grant, scope, expiry, and viewer
binding rules are described in :doc:`x-oauth`.

Verify this documentation locally:

.. code-block:: console

   docker build --pull --progress=plain -f docs/Dockerfile -t x-img-docs:check .
   docker run --rm x-img-docs:check
