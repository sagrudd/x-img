# Pinakotheke 1.0.0

Pinakotheke 1.0.0 completes the coordinated migration from the `x-img`
planning identity. The public repository, Rust workspace, packages, Firefox
listing, Monas product, DASObjectStore application, and documentation now use
the canonical Pinakotheke identity together. Historic `x-img` schema names and
the warning-emitting `x-img` CLI remain supported compatibility interfaces.

## Release contents

- DEB and RPM packages for x86_64 and arm64.
- Unsigned macOS PKGs for x86_64 and arm64.
- Deterministic unsigned Firefox XPIs labelled for macOS, Windows, and Linux on
  x86_64 and arm64. The extension files are platform-independent and retain the
  existing Gecko extension ID.
- `SHA256SUMS`, `release-manifest.v1.json`, and a CycloneDX 1.6 SBOM.
- Tested x-img 0.9.0 → Pinakotheke 1.0.0 → x-img 0.9.0 DEB/RPM transitions on
  both architectures with exact authority/catalogue metadata preservation.

All artifacts explicitly report `signed: false`. Apple notarization and Mozilla
Add-ons signing were not performed because release credentials are not present.
Review platform installation warnings and verify checksums before installation.

## Operational boundaries

- Native packages contain the CLI, MPL license, and host-composable Monas
  bootstrap. Monas owns login and authenticated host context; DASObjectStore is
  the only durable media authority.
- X functionality requires official authorization plus the documented policy
  and host-secret boundaries. Fixture-tested contracts do not imply that a
  particular live account or API entitlement is available.
- Instagram is an explicitly enabled ordinary website through the Firefox
  observed-media path; no dedicated Instagram API connector is required.
- Website capture is opt-in. Only displayed thumbnails and explicitly opened
  originals are eligible, and origin loading remains the fail-open fallback.
- Video normalization requires an authorized digest-pinned Docker/FFmpeg worker
  and a supported playback profile. DRM circumvention is not supported.
- GEO, SRA, ENA, and NCBI acquisition accepts bounded, explicit, reviewed
  resources only and does not crawl repositories.
- Hosted CI was not used. Local Rust, Firefox, package, contract, audit,
  fault-recovery, transition, and pinned Sphinx-container checks are the release
  authority.

## Verification

Download the desired artifacts with `SHA256SUMS`, then run:

```console
shasum -a 256 -c SHA256SUMS
```

The checksum file covers thirteen artifacts. The typed release manifest records
platform, architecture, byte length, checksum, and signing state. The
`pinakotheke-1.0.0.cdx.json` file records the software inventory.

Report defects at <https://github.com/sagrudd/pinakotheke/issues>.
