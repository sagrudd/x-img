# ADR 0011: Pinakotheke v1.0 identity migration

- Status: Accepted as the XIMG-009 release plan; execution is permitted only
  by the v1.0.0 release gate
- Date: 2026-07-14
- Deciders: x-img maintainers
- Scope: product naming, repository, package, host, extension, persistent
  identity, compatibility, rollback, and release evidence

## Context

`x-img` is the public planning repository name. The v1.0.0 product is
**Pinakotheke**, with target repository `sagrudd/pinakotheke`. A cosmetic
rename is unsafe: product IDs appear in host mounts, application identities,
extension pairings, object provenance, URLs, and release artefacts. Rewriting
object references or versioned schema identifiers merely to make them read like
a brand would risk data loss and ambiguous historical audit records.

The migration is one release-gated, reversible deployment plan, not a series of
partial renames. Its machine-readable inventory and proof contract is
[`docs/fixtures/pinakotheke-identity-migration-matrix.json`](../fixtures/pinakotheke-identity-migration-matrix.json).
It is synthetic and has no account, endpoint, ObjectStore, or credential data.

The compatibility-sensitive sibling snapshots inspected for this plan are:

| Boundary | Commit | Identity concern |
| --- | --- | --- |
| Monas | `3d21b0bc7b83fa8408d01b93347a56f43f3a96b7` | product ID, web/API mount, product root, and host session boundary |
| DASObjectStore | `3d882a261185cfcb0e4590c440aca64aa00a17e7` | daemon-owned service principal, audience, scope, audit identity, and object reference |
| Mnemosyne/Synoptikon | `ee21d98b23dec3caa6926d9d0dcc002989aa465b` | future product manifest, host adapter, and catalogue identity |
| Mnemosyne design language | `5539df8f662a78ebdf7cf4c868d71831380c8cfd` | product-leading label with Mnemosyne provenance footer |

These are inspection pins only. The public build remains independent of sibling
checkouts and consumes published contracts or synthetic fixtures.

GitHub redirects web and Git URLs after a repository rename, but it does not
redirect GitHub Pages or calls to an action hosted by a renamed repository, and
recreating the old name removes redirects. Redirects are therefore a safety net,
not a compatibility interface. Sources: [GitHub repository rename
guidance](https://docs.github.com/en/enterprise-cloud%40latest/repositories/creating-and-managing-repositories/renaming-a-repository)
and [GitHub repository transfer guidance](https://docs.github.com/en/repositories/creating-and-managing-repositories/transferring-a-repository).

## Decisions

### Brand labels versus stable technical identity

At v1.0.0, the human-facing product name, repository, executable, package,
Monas product label, Synoptikon label, extension listing, documentation title,
release artefacts, and support text become **Pinakotheke**. New product IDs and
new integration registrations use `pinakotheke`.

The following are not rewritten solely for branding:

- existing `x-img.*` schema identifiers and accepted schema-major readers;
- existing catalogue IDs, canonical media identities, object keys, checksums,
  object references, immutable provenance events, and audit records;
- a published Firefox `gecko.id`; changing it creates a distinct extension and
  strands updates and pairings; and
- existing DASObjectStore service-principal IDs in historical audit records.

New schemas may use `pinakotheke.*` only with an explicit versioned migration.
A display label is never an identity key.

### Compatibility and deprecation window

`x-img` is a documented legacy alias, not a second product. During v1.x, legacy
repository redirects, local Git remotes, a CLI wrapper, Monas mount/product
routes, configuration/product labels, and pre-v1 extension-pairing records stay
available where they already exist. They emit redacted migration notices and
never expose session, ObjectStore, or browsing data.

The minimum window is the later of twelve months after v1.0.0 or two subsequent
minor releases (through at least v1.2.0). Removal is considered only in v2.0.0
after measured usage, a deprecation notice, and the proof suite below. Earlier
removal needs a security, legal, privacy, or data-integrity reason and a safe
export/recovery path.

### Versioned data and pairing migration

Migration is copy-on-write and idempotent. A migration record binds legacy and
canonical product identity, schema/version, actor/session reference, time,
result, and checksum/reference where applicable. It never mutates a committed
media object or rekeys an existing DASObjectStore object for branding.

- **Configuration:** existing `x-img.*.v1` schemas remain readable. A
  migrator may add canonical identity fields only with a backup and deterministic
  report.
- **Catalogue/provenance:** historical records remain readable; new display and
  adapter labels are additive.
- **DASObjectStore:** register a scoped `pinakotheke` principal only after scope
  review. Legacy credentials are rotated/revoked at the authority boundary;
  historic principal IDs and object references remain intact.
- **Monas/Synoptikon:** register canonical product manifests/mounts, then use a
  host-controlled legacy alias. Authentication remains host-owned.
- **Firefox:** choose a Pinakotheke ID before first public signing. If a pre-v1
  signed ID exists, retain it and change only listing/brand metadata; re-pairing
  is explicit, reviewed, and never silently changes an ObjectStore.

### Release sequence and no-partial gate

1. **Preflight:** confirm the target slug; inventory GitHub settings, Pages,
   packages, webhooks, actions, releases, branch rules, clone remotes, published
   crates, AMO identity, host registrations, DAS principals, and controlled
   documentation. Missing package inventory scope is a release blocker.
2. **Compatibility release:** deliver readers, aliases, tools, backups,
   warnings, Sphinx guidance, and synthetic proof fixtures in v0.9.x/RC. No
   public rename happens while a proof fails.
3. **v1 cutover:** take verified backup/export; drain migration jobs; tag the
   pre-cutover commit; publish canonical metadata; rename GitHub; update remotes,
   links, host registrations, DAS principal, and extension listing; run the
   full suite; tag `v1.0.0` from the verified canonical commit.
4. **Postflight:** verify canonical URL and old redirect, update controlled
   clones, verify aliases, monitor migration audit failures, retain rollback
   artefacts, and publish the deprecation timetable.

No phase may be advertised as Pinakotheke v1.0.0. The release blocks if any
canonical identity, required alias, backup, proof, documentation, host
registration, ObjectStore scope, or extension pairing check is missing.

### Rollback

Before the GitHub rename and v1 tag, rollback restores the preflight artefact
and leaves legacy data untouched. After rename, do not recreate
`sagrudd/x-img`: it would destroy the redirect. Revert deployment/host
registrations only, retain the canonical repository, and serve
legacy-compatible readers. For data, select the compatible reader or recorded
backup and emit an audit event; never delete or mutate committed objects.

## Required proof suite

XIMG-081 and XIMG-086 turn these synthetic planning cases into native,
contract, and real-Firefox tests before v1.0.0:

| Case | Required proof |
| --- | --- |
| Existing configuration | Legacy `x-img.*.v1` config reads unchanged or migrates idempotently with backup, report, and unknown-major rejection. |
| Existing catalogue/provenance | Historical labels, canonical identities, checksums, and audit events remain queryable; new display labels do not rewrite them. |
| Existing ObjectStore reference | Object key, endpoint ID, ObjectStore ID, checksum, range access, and authorization remain stable. |
| Existing extension pairing | A legacy pairing needs explicit reviewed re-pair; revocation, origin binding, expiry, and destination safeguards remain intact. |
| CLI/package/API | Canonical entry points work; aliases warn and preserve arguments/response compatibility for the window. |
| Host adapters | Canonical/legacy mounts authenticate with host context and never create direct login. |
| Repository/release | Canonical URL, release artefacts, SBOM, checksums, notes, and old redirect are observed without relying on redirect. |

The fixture check proves plan coverage only; it does not replace implementation
tests.

## Consequences

The project carries aliases/readers longer than a one-off rename, but durable
data and browser installation identity remain safe. New code uses canonical IDs
plus explicit versioned aliases; it does not infer identity from repository URL,
display name, or the first ObjectStore. After v1, UI and documentation lead with
Pinakotheke, showing `x-img` only where an operator needs migration guidance.
