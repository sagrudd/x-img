Local Pinakotheke monolith
==========================

``pinakotheke-monolith`` is the local-first distribution framework for a
coherent Pinakotheke web service. It does not merge security authorities:
Monas/Prosopikon continues to own login and sessions, and DASObjectStore
continues to own all durable media bytes. The foreground service now recognizes
a reviewed DASObjectStore-managed local profile while Monas authentication
remains visibly unavailable.

Start on macOS
--------------

Run as the ordinary local user, never with ``sudo``:

.. code-block:: console

   cargo run -p pinakotheke-cli --bin pinakotheke -- serve

The default listener is ``http://127.0.0.1:8731`` and the default product root
is ``~/.x-img``. An explicit alternative must be absolute:

.. code-block:: console

   pinakotheke serve --root "$HOME/.x-img" --port 8731

Startup creates only private mode-``0700`` metadata directories:

.. code-block:: text

   ~/.x-img/
     config/
     state/
     run/
     logs/

The root and its required children must be real directories, not symlinks.
Provision local ObjectStore authority
-------------------------------------

Docker Desktop must be running. Review the plan, then ask DASObjectStore's
canonical helper to provision the bounded profile:

.. code-block:: console

   pinakotheke storage local-profile plan \
     --provisioner /absolute/path/to/DASObjectStore/deploy/local-docker/local.sh
   pinakotheke storage local-profile provision \
     --provisioner /absolute/path/to/DASObjectStore/deploy/local-docker/local.sh

The authority owns ``~/.x-img/dasobjectstore`` and the named
``pinakotheke_local`` logical ObjectStore. Private Garage configuration and
keys remain under ``~/.config/dasobjectstore``. Pinakotheke stores only the
secret-free stable endpoint/ObjectStore selection in its metadata state; it
never writes media by treating the managed root as an ordinary folder.

Use ``status`` to re-discover and validate the authority identity, or ``down``
to stop the containers without deleting state. Both accept the same required
``--provisioner`` argument. An alternate ``--root`` must be absolute.

Readiness
---------

``GET /health`` reports coarse process liveness. ``GET /ready`` reports three
worded component states. In the first slice, ``pinakotheke`` is ``Ready`` while
``monas_authentication`` is ``Not configured``. ``dasobjectstore`` becomes
``Ready`` only when the exact secret-free managed selection has been persisted;
the overall state remains ``not_ready`` until Monas is composed. Authenticated
product and media routes are not mounted yet.

Stop the foreground process with ``Control-C``. Axum stops accepting new work
and completes graceful shutdown.

Network safety
--------------

Loopback is the default and recommended binding. A non-loopback address is
refused unless the operator supplies the deliberately explicit
``--allow-non-loopback-without-authentication`` acknowledgement. The option is
for controlled development only: it prints a warning and does not create TLS or
authentication. Do not expose this first slice to an untrusted network.

Next slices
-----------

XIMG-092 composes Monas/Prosopikon authentication and host context.
XIMG-093 adds per-user macOS ``launchd`` management, and XIMG-094 proves a
clean-home authenticated ingest/read/restart flow end to end.
