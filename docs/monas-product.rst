Monas product registration
==========================

x-img is registered as one authenticated Monas product. The versioned public
registration is ``contracts/monas/x-img-product-bootstrap.v1.json``. It was
shaped against ``../monas`` commit
``3d21b0bc7b83fa8408d01b93347a56f43f3a96b7``; that inspection pin is not a
build dependency.

Mount and authority boundary
----------------------------

The registration has exactly one application mount,
``/products/x-img/app/``, and one API mount,
``/products/x-img/api/``. Its product root is ``/opt/x-img``. The product
requires both Monas host authentication and DASObjectStore availability.

Monas and its Prosopikon integration own login, logout, registration, session
cookies, session issuance, and session verification. x-img does not declare a
login endpoint and must never accept passwords, browser cookies, or session
tokens in its configuration or browser extension rules. The next host-context
adapter task validates the host-provided identity at the actual Axum boundary;
until that adapter exists, this registration is a strict planning and fixture
gate rather than a live authentication service.

The required capability list makes the boundaries visible: host-mandated web
authentication, DASObjectStore use, catalogue review, account refresh, browser
capture, and bioinformatics plan review. It does not grant local filesystem
media storage or an independent account system.

Future host equivalence
-----------------------

The same bootstrap declares ``monas_standalone`` and
``synoptikon_integrated`` as supported host modes and identifies
``mnemosyne.product_ui.bootstrap.v1`` as its future equivalent bootstrap
schema. In either mode, the host owns authenticated context; the x-img domain
and connector boundaries remain host-neutral.

Validation fixtures
-------------------

``x-img-core`` validates the contract in its native test suite. Synthetic
negative fixtures prove that a product declaring anonymous API access or a
direct x-img login route is rejected. They do not contain credentials, cookies,
or any real host endpoint.
