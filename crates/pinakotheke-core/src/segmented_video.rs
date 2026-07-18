// SPDX-License-Identifier: MPL-2.0
//! Bounded, metadata-only planning and fail-closed substitution for segmented media.
//!
//! The planner accepts identities for resources Firefox already observed while
//! the user played media. It never fetches a manifest, enumerates a playlist,
//! carries a URL or authorization value, or rewrites origin playback.

#![allow(missing_docs)]

use crate::video_profile::NormalizedVideoState;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;

pub const SEGMENTED_GATE_SCHEMA: &str = "x-img.segmented-video-gate.v1";
pub const SEGMENTED_PLAN_SCHEMA: &str = "x-img.segmented-media-plan.v1";
pub const MAX_OBSERVED_SEGMENTS: usize = 256;
pub const MAX_CODEC_DIAGNOSTICS: usize = 16;
pub const MAX_DECLARED_BYTES: u64 = 16 * 1024 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ManifestKind {
    Hls,
    Dash,
    Mse,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PolicyDecision {
    Allowed,
    Blocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProtectionState {
    Clear,
    Encrypted,
    Drm,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObservedSegment {
    /// SHA-256 of the adapter's canonical, query-free segment identity.
    pub identity_sha256: String,
    pub ordinal: u32,
    pub declared_bytes: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CodecDiagnostic {
    /// A short codec family such as `avc1`, never raw probe output.
    pub codec: String,
    /// A short container family such as `mpeg_ts` or `fmp4`.
    pub container: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SegmentedObservation {
    pub schema_version: String,
    pub adapter_id: String,
    pub adapter_version: String,
    pub origin: String,
    pub manifest_kind: ManifestKind,
    pub user_played: bool,
    pub hidden_traversal: bool,
    pub authorization_context_observed: bool,
    pub policy: PolicyDecision,
    pub protection: ProtectionState,
    /// SHA-256 of the canonical manifest/presentation identity.
    pub manifest_identity_sha256: String,
    pub observed_segments: Vec<ObservedSegment>,
    pub diagnostics: Vec<CodecDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SegmentedAdapterProof {
    pub adapter_id: String,
    pub adapter_version: String,
    pub manifest_kind: ManifestKind,
    pub manifest_canonicalization_version: String,
    pub segment_canonicalization_version: String,
    pub synthetic_fixture_id: String,
    pub firefox_evidence_id: String,
    pub proves_identity: bool,
    pub proves_retry_idempotency: bool,
    pub proves_policy_and_drm_blocks: bool,
    pub proves_fail_open: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SegmentedAcquisitionPlan {
    pub schema_version: &'static str,
    pub plan_id: String,
    pub adapter_id: String,
    pub adapter_version: String,
    pub manifest_kind: ManifestKind,
    pub manifest_identity_sha256: String,
    pub segment_count: usize,
    pub declared_bytes: Option<u64>,
    pub diagnostics: Vec<CodecDiagnostic>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SegmentedPlanBlock {
    InvalidRequest,
    NotUserPlayed,
    HiddenTraversal,
    AuthorizationContext,
    PolicyBlocked,
    DrmOrEncryption,
    EvidenceMissingOrMismatch,
    EvidenceIncomplete,
    LimitExceeded,
    DuplicateOrUnorderedSegment,
}

/// Produce a deterministic plan from already-observed, bounded identities.
/// Any block leaves ordinary origin playback untouched.
pub fn plan_segmented_acquisition(
    observation: &SegmentedObservation,
    proof: &SegmentedAdapterProof,
) -> Result<SegmentedAcquisitionPlan, SegmentedPlanBlock> {
    if observation.schema_version != SEGMENTED_PLAN_SCHEMA
        || !https_origin(&observation.origin)
        || !identifier(&observation.adapter_id)
        || !semver(&observation.adapter_version)
        || !sha256(&observation.manifest_identity_sha256)
    {
        return Err(SegmentedPlanBlock::InvalidRequest);
    }
    if !observation.user_played {
        return Err(SegmentedPlanBlock::NotUserPlayed);
    }
    if observation.hidden_traversal {
        return Err(SegmentedPlanBlock::HiddenTraversal);
    }
    if observation.authorization_context_observed {
        return Err(SegmentedPlanBlock::AuthorizationContext);
    }
    if observation.policy != PolicyDecision::Allowed {
        return Err(SegmentedPlanBlock::PolicyBlocked);
    }
    if observation.protection != ProtectionState::Clear {
        return Err(SegmentedPlanBlock::DrmOrEncryption);
    }
    if proof.adapter_id != observation.adapter_id
        || proof.adapter_version != observation.adapter_version
        || proof.manifest_kind != observation.manifest_kind
        || !semver(&proof.manifest_canonicalization_version)
        || !semver(&proof.segment_canonicalization_version)
        || !identifier(&proof.synthetic_fixture_id)
        || !identifier(&proof.firefox_evidence_id)
    {
        return Err(SegmentedPlanBlock::EvidenceMissingOrMismatch);
    }
    if !proof.proves_identity
        || !proof.proves_retry_idempotency
        || !proof.proves_policy_and_drm_blocks
        || !proof.proves_fail_open
    {
        return Err(SegmentedPlanBlock::EvidenceIncomplete);
    }
    if observation.observed_segments.is_empty()
        || observation.observed_segments.len() > MAX_OBSERVED_SEGMENTS
        || observation.diagnostics.len() > MAX_CODEC_DIAGNOSTICS
        || observation
            .diagnostics
            .iter()
            .any(|item| !diagnostic(&item.codec) || !diagnostic(&item.container))
    {
        return Err(SegmentedPlanBlock::LimitExceeded);
    }

    let mut expected = 0_u32;
    let mut total = Some(0_u64);
    let mut identities = HashSet::with_capacity(observation.observed_segments.len());
    let mut digest = Sha256::new();
    digest.update(observation.adapter_id.as_bytes());
    digest.update([0]);
    digest.update(observation.adapter_version.as_bytes());
    digest.update([0]);
    digest.update(observation.origin.as_bytes());
    digest.update([0]);
    digest.update(match observation.manifest_kind {
        ManifestKind::Hls => b"hls".as_slice(),
        ManifestKind::Dash => b"dash".as_slice(),
        ManifestKind::Mse => b"mse".as_slice(),
    });
    digest.update([0]);
    digest.update(proof.manifest_canonicalization_version.as_bytes());
    digest.update([0]);
    digest.update(proof.segment_canonicalization_version.as_bytes());
    digest.update([0]);
    digest.update(observation.manifest_identity_sha256.as_bytes());
    for segment in &observation.observed_segments {
        if segment.ordinal != expected
            || !sha256(&segment.identity_sha256)
            || !identities.insert(&segment.identity_sha256)
        {
            return Err(SegmentedPlanBlock::DuplicateOrUnorderedSegment);
        }
        expected = expected.saturating_add(1);
        digest.update(segment.ordinal.to_be_bytes());
        digest.update(segment.identity_sha256.as_bytes());
        match segment.declared_bytes {
            Some(bytes) => {
                digest.update([1]);
                digest.update(bytes.to_be_bytes());
            }
            None => digest.update([0]),
        }
        total = match (total, segment.declared_bytes) {
            (Some(sum), Some(bytes)) => Some(
                sum.checked_add(bytes)
                    .ok_or(SegmentedPlanBlock::LimitExceeded)?,
            ),
            _ => None,
        };
        if total.is_some_and(|bytes| bytes > MAX_DECLARED_BYTES) {
            return Err(SegmentedPlanBlock::LimitExceeded);
        }
    }

    Ok(SegmentedAcquisitionPlan {
        schema_version: SEGMENTED_PLAN_SCHEMA,
        plan_id: format!("sha256:{:x}", digest.finalize()),
        adapter_id: observation.adapter_id.clone(),
        adapter_version: observation.adapter_version.clone(),
        manifest_kind: observation.manifest_kind,
        manifest_identity_sha256: observation.manifest_identity_sha256.clone(),
        segment_count: observation.observed_segments.len(),
        declared_bytes: total,
        diagnostics: observation.diagnostics.clone(),
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SegmentedAdapterEvidence {
    pub adapter_id: String,
    pub adapter_version: String,
    pub origin: String,
    pub manifest_kind: ManifestKind,
    pub manifest_canonicalization_version: String,
    pub segment_canonicalization_version: String,
    pub normalized_profile_id: String,
    pub synthetic_fixture_id: String,
    pub firefox_evidence_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SegmentedGateRequest {
    pub schema_version: String,
    pub origin: String,
    pub adapter_id: String,
    pub adapter_version: String,
    pub manifest_kind: ManifestKind,
    pub displayed_by_user: bool,
    pub explicitly_opened: bool,
    pub drm_or_encrypted: bool,
    pub normalized_state: NormalizedVideoState,
    pub normalized_profile_id: String,
    pub evidence: Option<SegmentedAdapterEvidence>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SegmentedGateReason {
    InvalidRequest,
    NotDisplayedOrOpened,
    DrmOrEncryption,
    AdapterEvidenceMissing,
    AdapterEvidenceMismatch,
    NormalizedRenditionNotReady,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SegmentedGateDecision {
    AdapterApproved,
    OriginServed(SegmentedGateReason),
}

#[must_use]
pub fn evaluate_segmented_substitution(request: &SegmentedGateRequest) -> SegmentedGateDecision {
    if request.schema_version != SEGMENTED_GATE_SCHEMA
        || !https_origin(&request.origin)
        || !identifier(&request.adapter_id)
        || !semver(&request.adapter_version)
        || !identifier(&request.normalized_profile_id)
    {
        return SegmentedGateDecision::OriginServed(SegmentedGateReason::InvalidRequest);
    }
    if !request.displayed_by_user || !request.explicitly_opened {
        return SegmentedGateDecision::OriginServed(SegmentedGateReason::NotDisplayedOrOpened);
    }
    if request.drm_or_encrypted {
        return SegmentedGateDecision::OriginServed(SegmentedGateReason::DrmOrEncryption);
    }
    if request.normalized_state != NormalizedVideoState::Ready {
        return SegmentedGateDecision::OriginServed(
            SegmentedGateReason::NormalizedRenditionNotReady,
        );
    }
    let Some(evidence) = &request.evidence else {
        return SegmentedGateDecision::OriginServed(SegmentedGateReason::AdapterEvidenceMissing);
    };
    if evidence.adapter_id != request.adapter_id
        || evidence.adapter_version != request.adapter_version
        || evidence.origin != request.origin
        || evidence.manifest_kind != request.manifest_kind
        || evidence.normalized_profile_id != request.normalized_profile_id
        || !semver(&evidence.manifest_canonicalization_version)
        || !semver(&evidence.segment_canonicalization_version)
        || !identifier(&evidence.synthetic_fixture_id)
        || !identifier(&evidence.firefox_evidence_id)
    {
        return SegmentedGateDecision::OriginServed(SegmentedGateReason::AdapterEvidenceMismatch);
    }
    SegmentedGateDecision::AdapterApproved
}

fn identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b':' | b'-'))
}

fn diagnostic(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 32
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

fn semver(value: &str) -> bool {
    let mut parts = value.split('.');
    matches!((parts.next(), parts.next(), parts.next(), parts.next()), (Some(a), Some(b), Some(c), None) if [a, b, c].iter().all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit())))
}

fn sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn https_origin(value: &str) -> bool {
    value.strip_prefix("https://").is_some_and(|host| {
        !host.is_empty() && !host.contains(['/', '@', '?', '#', '*', ' ', '\n', '\r'])
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    fn observation(kind: ManifestKind) -> SegmentedObservation {
        SegmentedObservation {
            schema_version: SEGMENTED_PLAN_SCHEMA.into(),
            adapter_id: "generic-observed-v1".into(),
            adapter_version: "1.0.0".into(),
            origin: "https://media.example.invalid".into(),
            manifest_kind: kind,
            user_played: true,
            hidden_traversal: false,
            authorization_context_observed: false,
            policy: PolicyDecision::Allowed,
            protection: ProtectionState::Clear,
            manifest_identity_sha256: "a".repeat(64),
            observed_segments: vec![
                ObservedSegment {
                    identity_sha256: "b".repeat(64),
                    ordinal: 0,
                    declared_bytes: Some(1024),
                },
                ObservedSegment {
                    identity_sha256: "c".repeat(64),
                    ordinal: 1,
                    declared_bytes: Some(2048),
                },
            ],
            diagnostics: vec![CodecDiagnostic {
                codec: "avc1".into(),
                container: "fmp4".into(),
            }],
        }
    }

    fn proof(kind: ManifestKind) -> SegmentedAdapterProof {
        SegmentedAdapterProof {
            adapter_id: "generic-observed-v1".into(),
            adapter_version: "1.0.0".into(),
            manifest_kind: kind,
            manifest_canonicalization_version: "1.0.0".into(),
            segment_canonicalization_version: "1.0.0".into(),
            synthetic_fixture_id: "segmented-matrix-v1".into(),
            firefox_evidence_id: "firefox-play-v1".into(),
            proves_identity: true,
            proves_retry_idempotency: true,
            proves_policy_and_drm_blocks: true,
            proves_fail_open: true,
        }
    }

    #[test]
    fn hls_dash_and_mse_plans_are_deterministic() {
        for kind in [ManifestKind::Hls, ManifestKind::Dash, ManifestKind::Mse] {
            let first = plan_segmented_acquisition(&observation(kind), &proof(kind)).unwrap();
            let retry = plan_segmented_acquisition(&observation(kind), &proof(kind)).unwrap();
            assert_eq!(first, retry);
            assert_eq!(first.declared_bytes, Some(3072));
            assert_eq!(first.segment_count, 2);
        }
    }

    #[test]
    fn blocks_policy_drm_authorization_hidden_and_unplayed_inputs() {
        let base = observation(ManifestKind::Hls);
        let evidence = proof(ManifestKind::Hls);
        for (mut input, expected) in [
            (
                {
                    let mut v = base.clone();
                    v.user_played = false;
                    v
                },
                SegmentedPlanBlock::NotUserPlayed,
            ),
            (
                {
                    let mut v = base.clone();
                    v.hidden_traversal = true;
                    v
                },
                SegmentedPlanBlock::HiddenTraversal,
            ),
            (
                {
                    let mut v = base.clone();
                    v.authorization_context_observed = true;
                    v
                },
                SegmentedPlanBlock::AuthorizationContext,
            ),
            (
                {
                    let mut v = base.clone();
                    v.policy = PolicyDecision::Blocked;
                    v
                },
                SegmentedPlanBlock::PolicyBlocked,
            ),
            (
                {
                    let mut v = base.clone();
                    v.protection = ProtectionState::Drm;
                    v
                },
                SegmentedPlanBlock::DrmOrEncryption,
            ),
        ] {
            assert_eq!(plan_segmented_acquisition(&input, &evidence), Err(expected));
            input.user_played = true;
        }
    }

    #[test]
    fn blocks_unbounded_duplicate_and_incomplete_evidence() {
        let mut input = observation(ManifestKind::Dash);
        input.observed_segments[1].ordinal = 0;
        assert_eq!(
            plan_segmented_acquisition(&input, &proof(ManifestKind::Dash)),
            Err(SegmentedPlanBlock::DuplicateOrUnorderedSegment)
        );
        let mut duplicate = observation(ManifestKind::Dash);
        duplicate.observed_segments[1].identity_sha256 = "b".repeat(64);
        assert_eq!(
            plan_segmented_acquisition(&duplicate, &proof(ManifestKind::Dash)),
            Err(SegmentedPlanBlock::DuplicateOrUnorderedSegment)
        );
        let mut evidence = proof(ManifestKind::Dash);
        evidence.proves_fail_open = false;
        assert_eq!(
            plan_segmented_acquisition(&observation(ManifestKind::Dash), &evidence),
            Err(SegmentedPlanBlock::EvidenceIncomplete)
        );
    }

    fn gate_request() -> SegmentedGateRequest {
        SegmentedGateRequest {
            schema_version: SEGMENTED_GATE_SCHEMA.into(),
            origin: "https://media.example.invalid".into(),
            adapter_id: "synthetic-hls".into(),
            adapter_version: "1.2.0".into(),
            manifest_kind: ManifestKind::Hls,
            displayed_by_user: true,
            explicitly_opened: true,
            drm_or_encrypted: false,
            normalized_state: NormalizedVideoState::Ready,
            normalized_profile_id: "pinakotheke-video-mp4-v1".into(),
            evidence: Some(SegmentedAdapterEvidence {
                adapter_id: "synthetic-hls".into(),
                adapter_version: "1.2.0".into(),
                origin: "https://media.example.invalid".into(),
                manifest_kind: ManifestKind::Hls,
                manifest_canonicalization_version: "1.0.0".into(),
                segment_canonicalization_version: "1.0.0".into(),
                normalized_profile_id: "pinakotheke-video-mp4-v1".into(),
                synthetic_fixture_id: "fixture-hls-1".into(),
                firefox_evidence_id: "firefox-hls-1".into(),
            }),
        }
    }

    #[test]
    fn substitution_still_requires_ready_exact_evidence() {
        assert_eq!(
            evaluate_segmented_substitution(&gate_request()),
            SegmentedGateDecision::AdapterApproved
        );
        let mut drm = gate_request();
        drm.drm_or_encrypted = true;
        assert_eq!(
            evaluate_segmented_substitution(&drm),
            SegmentedGateDecision::OriginServed(SegmentedGateReason::DrmOrEncryption)
        );
    }

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct FixtureMatrix {
        schema_version: String,
        cases: Vec<FixtureCase>,
    }

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct FixtureCase {
        id: String,
        expected: String,
        observation: SegmentedObservation,
        proof: SegmentedAdapterProof,
    }

    #[test]
    fn bounded_synthetic_matrix_proves_planning_and_fail_open_blocks() {
        let matrix: FixtureMatrix = serde_json::from_str(include_str!(
            "../../../fixtures/segmented-media/v1/cases.json"
        ))
        .unwrap();
        assert_eq!(matrix.schema_version, "x-img.segmented-media-fixtures.v1");
        for case in matrix.cases {
            let actual = plan_segmented_acquisition(&case.observation, &case.proof);
            match case.expected.as_str() {
                "planned" => {
                    let first = actual.unwrap_or_else(|error| panic!("{}: {error:?}", case.id));
                    let retry = plan_segmented_acquisition(&case.observation, &case.proof).unwrap();
                    assert_eq!(first, retry, "{}", case.id);
                }
                "policy_blocked" => assert_eq!(actual, Err(SegmentedPlanBlock::PolicyBlocked)),
                "drm_or_encryption" => {
                    assert_eq!(actual, Err(SegmentedPlanBlock::DrmOrEncryption));
                }
                "authorization_context" => {
                    assert_eq!(actual, Err(SegmentedPlanBlock::AuthorizationContext));
                }
                unexpected => panic!("{}: unknown expected value {unexpected}", case.id),
            }
        }
    }
}
