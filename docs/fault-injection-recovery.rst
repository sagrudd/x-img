Fault injection and recovery
============================

The release-facing fault suite exercises the failure boundaries shared by
acquisition, ObjectStore commit, scheduling, normalization, capture, and cache
substitution. It is deterministic, uses only synthetic metadata and ephemeral
test bytes, and never requires provider credentials or durable media fixtures.

Run the suite from the repository root:

.. code-block:: console

   scripts/faults/check.sh

The versioned matrix in ``docs/fixtures/fault-recovery-matrix.json`` binds each
fault to one observable safety invariant. The runner rejects unknown, missing,
or duplicated cases before executing the exact Rust or Firefox test associated
with every entry.

Recovery guarantees
-------------------

The suite establishes that:

* corrupt, short, or backpressured ingest does not become a verified commit;
* replay across authority crash boundaries converges on at most one immutable
  object and checksum disagreement remains a conflict;
* destination revalidation never switches endpoint or ObjectStore silently;
* cancellation and exhausted budgets release scheduler leases without falsely
  completing child work;
* interrupted normalization requires reconciliation and removes bounded
  ephemeral scratch after failure;
* an unavailable cache authority falls back to the origin instead of selecting
  another store;
* unavailable capture policy produces an explicit metadata failure without
  disturbing ordinary browsing; and
* Firefox restores the original image exactly once when substituted delivery
  fails CSP, CORS/CORP, MIME, length, or checksum-ETag validation.

The Firefox proof creates a temporary browser profile and loopback server, then
removes both. It does not use a user's Firefox profile, browsing history,
cookies, credentials, or downloaded media. These focused injections complement
the full local quality and documentation-container checks; hosted CI is not a
release dependency.
