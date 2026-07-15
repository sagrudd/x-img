Synoptikon catalogue integration
================================

Pinakotheke 1.1 adds a narrow read-only integration for Synoptikon. It does not
create a second login, object store, or public listener. Synoptikon authenticates
the request and injects tenant, account, project, entitlement, actor, and
correlation context before the product route is called.

Registration
------------

``contracts/synoptikon/pinakotheke-product-manifest.v1.json`` is the public
``mnemosyne.product.manifest.v1`` registration. It declares both Monas and
Synoptikon host modes, Synoptikon entitlement/account/audit ownership, and
DASObjectStore as the required artifact authority. The inspected contract
baseline is ``../mnemosyne`` commit
``52810176bf95a170f93d74a6f5daa94da5c6640e``. Pinakotheke has no unpublished
path dependency on that workspace.

Host context
------------

Synoptikon must issue the existing ``x-img.host-context.v1`` envelope with
``host_mode`` set to ``synoptikon_integrated`` and non-secret ``tenant_id``,
``account_id``, ``project_id``, and ``entitlement_id`` identifiers. Catalogue
access additionally requires ``ximg.catalogue.read``. Missing scope,
authorization, or authenticated context fails closed. Monas contexts reject
Synoptikon-only scope fields.

Catalogue projection
--------------------

The host composes ``router_with_synoptikon_catalogue`` below the manifest API
mount and exposes ``GET /api/synoptikon/v1/catalogue``. ``offset`` defaults to
zero and ``limit`` defaults to 100; a page may contain at most 200 records.
Ordering is deterministic by stable catalogue identifier, and records from any
other project are excluded before pagination.

The response contains review/media state, a display-safe source label,
discovery time, and the immutable endpoint, ObjectStore, object key, checksum,
content type, and length needed for governed artifact resolution. It never
contains media bytes, source URLs, browser history, cookies, credentials, or a
direct storage transport token. DASObjectStore remains the byte authority and
Synoptikon remains the tenant/project, entitlement, audit, and integrated-state
authority.

Verification
------------

Synthetic tests prove project isolation, stable bounded pagination, missing
authentication rejection, and the absence of source URLs. Validate the copied
wire contract against the inspected sibling checkout with:

.. code-block:: console

   scripts/contracts/check.sh --sibling mnemosyne

The normal local quality and containerized documentation checks remain release
authority; hosted CI is not required.
