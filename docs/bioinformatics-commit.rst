Confirmed bioinformatics commits
================================

XIMG-038 composes the reviewed bioinformatics plan, destination revalidation,
and bounded DASObjectStore ingest port for one explicitly selected file. It
does not resolve accessions, crawl repositories, pair devices, issue storage
credentials, or retain a payload file.

Before a commit
---------------

The plan must have an ``Allowed`` policy result and an explicit confirmation.
The selected file must be part of that plan, carry an exact byte count and a
SHA-256 checksum, and target the exact endpoint/ObjectStore pair that the user
reviewed. Immediately before bytes are accepted, the authority-state adapter
revalidates the stable IDs, labels, TLS trust, pairing expiry, ready state,
write capability, and quota. A changed or missing target is blocked; x-img
does not choose a replacement endpoint or ObjectStore.

Streaming and provenance
------------------------

Caller-provided chunks are passed directly to the bounded ingest port. The port
checks the declared length, incremental SHA-256, and authority completion
receipt before a provenance record is returned. It retains no dataset bytes.
An exact replay of the accession, file identity, checksum, and destination
returns the original metadata record without another transfer; a conflicting
destination is rejected.

The resulting metadata includes authority, accession/URL, release, file ID and
name, source checksum, transport, rights note, endpoint/ObjectStore IDs, object
key/reference, and commit time. Durable crash reconciliation and live provider
or DASObjectStore transport remain adapter work; the in-memory rule provides
the deterministic key and verified receipt boundary for those adapters.

Examples and failures
---------------------

For an explicit ``ENA:ERR400001`` file, first review the release, rights,
checksum, size, and endpoint plus ObjectStore; then confirm it. A checksum or
length mismatch, an expired pairing, a read-only or over-quota store, policy
block, or changed endpoint/store stops the commit before catalogue admission.
Retry only after the authority state and reviewed destination are repaired.

Verify this documentation locally:

.. code-block:: console

   docker build --pull --progress=plain -f docs/Dockerfile -t x-img-docs:check .
   docker run --rm x-img-docs:check
