# ADR 0005: User-selected video normalization and playback profiles

- Status: Proposed; no video-site implementation is authorized before policy,
  rights, and contract gates pass
- Date: 2026-07-14
- Deciders: x-img maintainers
- Scope: video-focused website selection, DASObjectStore ingest, normalization,
  playback, and Firefox substitution

## Context

Pinakotheke may support user-initiated discovery and selection of video from
video-focused websites with a DownloadThemAll-like review experience. That
workflow is a bounded selection tool, not a crawler. Only media actually
observed on an enabled page or explicitly selected by the user is eligible.
Site policy, rights, and the source authority remain binding; browser capture
does not avoid platform terms.

YouTube may appear in documentation only as an example of a policy-gated
adapter. No YouTube implementation or other site adapter may proceed without a
current terms and rights review that authorizes the exact behavior. There is no
DRM or encryption circumvention, authentication/cookie extraction, automatic
opening, hidden traversal, playlist/channel bulk acquisition by default, or
simulated browsing.

Current compatibility references for the profile decision:

- [Mozilla video codec guidance](https://developer.mozilla.org/en-US/docs/Web/Media/Guides/Formats/Video_codecs)
  describes WebM VP9/Opus, AV1/Opus, and MP4 H.264/AAC tradeoffs and browser
  compatibility; the acceptance matrix must be rerun against the supported
  Firefox versions before a profile is frozen.
- [Mozilla codec parameter guidance](https://developer.mozilla.org/en-US/docs/Web/Media/Guides/Formats/codecs_parameter)
  requires precise container/codec declarations in media metadata.
- [FFmpeg documentation](https://ffmpeg.org/ffmpeg.html) is the reference for
  the pinned converter invocation; arguments must be structured and never
  shell-interpolated. FFmpeg's [escaping guidance](https://www.ffmpeg.org/ffmpeg-all.html#Quoting-and-escaping)
  is not a substitute for avoiding a shell.
- [YouTube Terms](https://www.youtube.com/static?template=terms) are a policy
  gate for the named example; public visibility is not permission to download,
  retain, or transform content.

## Decisions

### Candidate selection and review

- A per-site opt-in adapter may report only candidates observed in the enabled
  page or explicitly selected by the user. It must not open pages, traverse
  hidden DOM/network state, crawl playlists/channels, or discover bulk media.
- The Monas-hosted task pane lists each candidate or representation with title,
  source URL/authority, duration, dimensions, source container and codecs,
  expected size when known, audio and subtitle tracks, policy/support result,
  reviewed endpoint plus ObjectStore, and intended normalization profile. The
  user chooses candidates and confirms the transfer plan before source ingest.
- Policy/rights, unsupported, DRM/encrypted, authentication-required, and
  incomplete candidates remain explicit `Blocked` or `Needs review` records
  with a reason and safe retry/profile-change action. The extension never
  supplies cookies, credentials, authorization headers, or site secrets.

### Versioned playback profile registry

The profile registry defines immutable, evidence-backed profiles rather than
choosing a codec by intuition:

| Profile ID | Container and tracks | Evaluation obligations |
| --- | --- | --- |
| `pinakotheke-video-webm-v1` | WebM, VP9/Opus; AV1/Opus is an explicitly measured candidate before adoption | Firefox version matrix, hardware acceleration, CPU/encoding cost, quality/size, seeking/ranges, licensing and distribution limits |
| `pinakotheke-video-mp4-v1` | MP4, H.264/AAC | Firefox version matrix, hardware acceleration, CPU/encoding cost, quality/size, seeking/ranges, patent/licensing and distribution limits |

The release candidate selects a default only after synthetic fixture playback,
Firefox browser coverage, probe validation, encoder benchmark, storage budget,
and rights/licensing evidence pass. A selected profile records its immutable
ID and version in every derived object and provenance manifest. A later codec,
encoder, or setting change creates a new profile version and never invalidates
the meaning of an old rendition. Until that evidence is accepted, no source
codec is advertised as a Pinakotheke playable default.

### Normalization, authority, and retention

- A candidate is not catalogue-ready or playable until a normalized rendition
  has been transcoded, committed to DASObjectStore, checksum-verified, probed,
  and tested in real Firefox playback. Cards show `Source selected`,
  `Normalizing`, `Ready`, `Blocked`, or `Failed`; source-only video is never
  shown as ready and the origin is not an unplayable fallback.
- A narrow containerized FFmpeg adapter performs normalization and probing with
  a pinned image digest and tool version. It accepts structured arguments,
  fixed output paths, bounded resources, and untrusted metadata as data. It
  records converter/image, profile ID/version, input/output codecs and
  settings, duration, dimensions, source/output checksums, logs/errors, and
  provenance without secrets.
- Source transfer and transcoding scratch use DASObjectStore-managed ingest
  staging or tightly bounded isolated/encrypted ephemeral worker scratch. It
  is deleted after success or failure and never becomes durable x-img-local
  storage. Derived normalized video, poster thumbnail, optional storyboard or
  contact sheet, subtitles/captions, and provenance manifest are separate
  typed DASObjectStore objects linked to the source identity.
- The normalized rendition is mandatory. Retaining the original source object
  is optional/configurable only where policy and rights permit. A source is
  never deleted before normalized commit and reconciliation are verified.

### Jobs and playback delivery

The bounded job model supports streaming/backpressure, cancellation, resumable
source transfer where possible, CPU/memory/disk quotas, queue backpressure,
phase progress (`Plan`, `Transfer`, `Normalize`, `Verify`, `Probe`, `Playback test`,
`Commit`), crash reconciliation, and idempotency keyed by source
identity/checksum plus profile ID/version. It never buffers an unbounded source
or output in memory.

Authorized playback of a committed rendition serves the recorded MIME type,
ETag, Content-Length, checksum, byte ranges, conditional requests, seek,
pause/resume, and authorization. Firefox tests must prove real range playback,
not only an HTTP 200 response. Failed normalization, unsupported codecs, DRM,
policy blocks, and rights uncertainty retain explicit terminal reasons and
safe retry/profile-change paths.

## Acceptance tests

- Fixture pages prove only observed or explicitly selected candidates enter the
  task pane; automatic opening, hidden traversal, bulk playlist/channel
  discovery, cookie/credential extraction, DRM bypass, and simulated browsing
  are rejected. Per-site capture/substitution remains opt-in, transparent, and
  fail-open.
- Task-pane fixtures render all candidate fields and target endpoint/ObjectStore
  together, support keyboard/focus behavior, require confirmation, and show
  policy/support/rights reasons in words.
- Synthetic redistributable fixtures evaluate WebM VP9/Opus, AV1/Opus, and MP4
  H.264/AAC for Firefox playback, hardware/software paths, ranges, seeking,
  quality, encoded size, CPU cost, storage cost, and licensing evidence. No
  copyrighted sample media is committed.
- Container-local tests use a pinned FFmpeg image/tool, structured argument
  passing, bounded CPU/memory/disk, no shell interpolation, deterministic
  probe/checksum manifests, cancellation, retry/resume, crash reconciliation,
  and cleanup of all scratch data.
- A source-only object cannot enter `Ready`; normalized video, poster,
  subtitles, optional storyboard, and provenance are separate typed objects,
  with source-retention policy and idempotency fixtures.
- Real Firefox tests prove MIME, ETag, Content-Length, conditional requests,
  byte ranges, seek/pause/resume, authorization, and fail-open behavior. An
  unsupported or failed normalization never serves an unplayable source as a
  ready rendition.
- A public clone builds without sibling-only path dependencies and without
  real site accounts, cookies, credentials, tokens, or copyrighted media.

## User-facing documentation

The Sphinx/Read the Docs project must explain candidate review, explicit
selection, rights/policy blocks, endpoint/ObjectStore confirmation, playback
profiles and their evidence, normalization phases, source retention, failure
states, Firefox playback checks, and the local container verification command.
The local `docs/Dockerfile` build remains authoritative.
