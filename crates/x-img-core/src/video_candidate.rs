// SPDX-License-Identifier: MPL-2.0
//! Explicit, metadata-only video candidate planning and codec-gap tracking.
//!
//! A candidate is created only from a user-observed or explicitly selected
//! video. It never fetches bytes, opens media, reads browser credentials, or
//! authorizes transfer. Confirmation produces only a later-worker reference.

#![allow(missing_docs)]

use std::collections::BTreeMap;

use crate::destination::ReviewedDestination;

pub const PINAKOTHEKE_FIREFOX_PLAYBACK_PROFILE: &str = "pinakotheke.firefox-h264-aac-mp4.v1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoCandidateRequest {
    pub candidate_id: String,
    pub origin: String,
    pub canonical_page_url: String,
    pub canonical_media_url: String,
    pub title: String,
    pub duration_seconds: u64,
    pub width: u32,
    pub height: u32,
    pub estimated_size_bytes: u64,
    pub container: String,
    pub video_codec: String,
    pub audio_codecs: Vec<String>,
    pub subtitle_languages: Vec<String>,
    pub adapter_id: String,
    pub adapter_version: String,
    pub observed_by_user: bool,
    pub explicitly_selected: bool,
    pub policy_allowed: bool,
    pub drm_protected: bool,
    pub segmented: bool,
    pub destination: ReviewedDestination,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CandidateState {
    AwaitingConfirmation,
    Confirmed,
    PolicyBlocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodecSupport {
    AlreadyFirefoxCompatible,
    RequiresNormalization,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoCandidatePlan {
    pub candidate_id: String,
    pub origin: String,
    pub canonical_page_url: String,
    pub canonical_media_url: String,
    pub title: String,
    pub duration_seconds: u64,
    pub width: u32,
    pub height: u32,
    pub estimated_size_bytes: u64,
    pub container: String,
    pub video_codec: String,
    pub audio_codecs: Vec<String>,
    pub subtitle_languages: Vec<String>,
    pub adapter_id: String,
    pub adapter_version: String,
    pub destination: ReviewedDestination,
    pub intended_playback_profile: &'static str,
    pub codec_support: CodecSupport,
    pub state: CandidateState,
    pub block_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodecGap {
    pub origin: String,
    pub container: String,
    pub video_codec: String,
    pub audio_codecs: Vec<String>,
    pub occurrences: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VideoCandidateError {
    InvalidMetadata,
    UnknownCandidate,
    NotConfirmable,
}

impl std::fmt::Display for VideoCandidateError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::InvalidMetadata => "video candidate metadata is invalid",
            Self::UnknownCandidate => "video candidate is not known",
            Self::NotConfirmable => "video candidate is not eligible for confirmation",
        })
    }
}

impl std::error::Error for VideoCandidateError {}

/// Candidate-plan state plus deduplicated codec-gap observations.
#[derive(Debug, Default)]
pub struct VideoCandidatePlanner {
    candidates: BTreeMap<String, VideoCandidatePlan>,
    codec_gaps: BTreeMap<String, CodecGap>,
}

impl VideoCandidatePlanner {
    /// Creates a reviewable candidate plan without acquiring any media bytes.
    pub fn plan(
        &mut self,
        request: VideoCandidateRequest,
    ) -> Result<&VideoCandidatePlan, VideoCandidateError> {
        validate(&request)?;
        let (state, codec_support, block_reason) = classification(&request);
        let plan = VideoCandidatePlan {
            candidate_id: request.candidate_id.clone(),
            origin: request.origin,
            canonical_page_url: request.canonical_page_url,
            canonical_media_url: request.canonical_media_url,
            title: request.title,
            duration_seconds: request.duration_seconds,
            width: request.width,
            height: request.height,
            estimated_size_bytes: request.estimated_size_bytes,
            container: request.container,
            video_codec: request.video_codec,
            audio_codecs: request.audio_codecs,
            subtitle_languages: request.subtitle_languages,
            adapter_id: request.adapter_id,
            adapter_version: request.adapter_version,
            destination: request.destination,
            intended_playback_profile: PINAKOTHEKE_FIREFOX_PLAYBACK_PROFILE,
            codec_support,
            state,
            block_reason,
        };
        if plan.codec_support == CodecSupport::RequiresNormalization {
            self.record_gap(&plan);
        }
        self.candidates.insert(plan.candidate_id.clone(), plan);
        Ok(self
            .candidates
            .get(&request.candidate_id)
            .expect("inserted candidate"))
    }

    /// Records the explicit user confirmation required before a future worker
    /// can create a transfer or normalization job.
    pub fn confirm(
        &mut self,
        candidate_id: &str,
    ) -> Result<&VideoCandidatePlan, VideoCandidateError> {
        let plan = self
            .candidates
            .get_mut(candidate_id)
            .ok_or(VideoCandidateError::UnknownCandidate)?;
        if plan.state != CandidateState::AwaitingConfirmation {
            return Err(VideoCandidateError::NotConfirmable);
        }
        plan.state = CandidateState::Confirmed;
        Ok(plan)
    }

    #[must_use]
    pub fn candidate(&self, candidate_id: &str) -> Option<&VideoCandidatePlan> {
        self.candidates.get(candidate_id)
    }

    /// Returns codec gaps in deterministic priority order: most observations,
    /// then stable codec metadata.
    #[must_use]
    pub fn codec_gaps(&self) -> Vec<&CodecGap> {
        let mut gaps: Vec<_> = self.codec_gaps.values().collect();
        gaps.sort_by(|left, right| {
            right
                .occurrences
                .cmp(&left.occurrences)
                .then_with(|| left.video_codec.cmp(&right.video_codec))
                .then_with(|| left.container.cmp(&right.container))
        });
        gaps
    }

    fn record_gap(&mut self, plan: &VideoCandidatePlan) {
        let key = format!(
            "{}:{}:{}:{}",
            plan.origin,
            plan.container,
            plan.video_codec,
            plan.audio_codecs.join(",")
        );
        let gap = self.codec_gaps.entry(key).or_insert(CodecGap {
            origin: plan.origin.clone(),
            container: plan.container.clone(),
            video_codec: plan.video_codec.clone(),
            audio_codecs: plan.audio_codecs.clone(),
            occurrences: 0,
        });
        gap.occurrences = gap.occurrences.saturating_add(1);
    }
}

fn classification(
    request: &VideoCandidateRequest,
) -> (CandidateState, CodecSupport, Option<String>) {
    if !request.observed_by_user && !request.explicitly_selected {
        return (
            CandidateState::PolicyBlocked,
            CodecSupport::Blocked,
            Some("candidate was neither observed nor explicitly selected by the user".into()),
        );
    }
    if !request.policy_allowed {
        return (
            CandidateState::PolicyBlocked,
            CodecSupport::Blocked,
            Some("site policy or rights do not permit acquisition".into()),
        );
    }
    if request.drm_protected {
        return (
            CandidateState::PolicyBlocked,
            CodecSupport::Blocked,
            Some("DRM-protected media is not supported".into()),
        );
    }
    if request.segmented {
        return (
            CandidateState::PolicyBlocked,
            CodecSupport::Blocked,
            Some("segmented media requires a proven site adapter".into()),
        );
    }
    if request.destination.object_type != "video" {
        return (
            CandidateState::PolicyBlocked,
            CodecSupport::Blocked,
            Some("reviewed destination is not compatible with video".into()),
        );
    }
    let compatible = request.container == "mp4"
        && matches!(request.video_codec.as_str(), "h264" | "avc1")
        && request.audio_codecs.iter().all(|codec| codec == "aac");
    (
        CandidateState::AwaitingConfirmation,
        if compatible {
            CodecSupport::AlreadyFirefoxCompatible
        } else {
            CodecSupport::RequiresNormalization
        },
        None,
    )
}

fn validate(request: &VideoCandidateRequest) -> Result<(), VideoCandidateError> {
    if !identifier(&request.candidate_id)
        || !identifier(&request.adapter_id)
        || !semver(&request.adapter_version)
        || !https_url(&request.origin, true)
        || !https_url(&request.canonical_page_url, false)
        || !https_url(&request.canonical_media_url, false)
        || request.title.is_empty()
        || request.title.len() > 240
        || request.duration_seconds == 0
        || request.width == 0
        || request.height == 0
        || request.estimated_size_bytes == 0
        || request.container.is_empty()
        || request.video_codec.is_empty()
        || request.audio_codecs.len() > 16
        || request.subtitle_languages.len() > 32
        || request.destination.endpoint_id.is_empty()
        || request.destination.object_store_id.is_empty()
    {
        return Err(VideoCandidateError::InvalidMetadata);
    }
    Ok(())
}

fn identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b':' | b'-'))
}

fn semver(value: &str) -> bool {
    let mut parts = value.split('.');
    matches!(
        (parts.next(), parts.next(), parts.next(), parts.next()),
        (Some(major), Some(minor), Some(patch), None)
            if [major, minor, patch].into_iter().all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
    )
}

fn https_url(value: &str, origin_only: bool) -> bool {
    value.starts_with("https://")
        && !value.contains(['?', '#', '@', ' ', '\n', '\r'])
        && value.len() <= 2_048
        && value[8..]
            .split('/')
            .next()
            .is_some_and(|host| !host.is_empty())
        && (!origin_only || !value[8..].contains('/'))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn destination() -> ReviewedDestination {
        ReviewedDestination {
            endpoint_id: "endpoint".into(),
            object_store_id: "store".into(),
            object_type: "video".into(),
            selection_kind: "site_override".into(),
            reviewed_at_unix_seconds: 1,
            actor_ref: "actor".into(),
        }
    }

    fn request() -> VideoCandidateRequest {
        VideoCandidateRequest {
            candidate_id: "candidate-0".into(),
            origin: "https://example.invalid".into(),
            canonical_page_url: "https://example.invalid/watch/one".into(),
            canonical_media_url: "https://cdn.example.invalid/one.webm".into(),
            title: "Synthetic observed video".into(),
            duration_seconds: 12,
            width: 1920,
            height: 1080,
            estimated_size_bytes: 12_000,
            container: "webm".into(),
            video_codec: "vp9".into(),
            audio_codecs: vec!["opus".into()],
            subtitle_languages: vec!["en".into()],
            adapter_id: "generic-video".into(),
            adapter_version: "1.0.0".into(),
            observed_by_user: true,
            explicitly_selected: false,
            policy_allowed: true,
            drm_protected: false,
            segmented: false,
            destination: destination(),
        }
    }

    #[test]
    fn observed_unusual_codec_requires_confirmation_and_records_a_priority_gap() {
        let mut planner = VideoCandidatePlanner::default();
        let plan = planner.plan(request()).expect("candidate").clone();
        assert_eq!(plan.state, CandidateState::AwaitingConfirmation);
        assert_eq!(plan.codec_support, CodecSupport::RequiresNormalization);
        assert_eq!(
            plan.intended_playback_profile,
            PINAKOTHEKE_FIREFOX_PLAYBACK_PROFILE
        );
        assert_eq!(planner.codec_gaps()[0].video_codec, "vp9");
        let confirmed = planner
            .confirm("candidate-0")
            .expect("explicit confirmation");
        assert_eq!(confirmed.state, CandidateState::Confirmed);
    }

    #[test]
    fn gaps_are_deduplicated_and_prioritized_by_occurrence() {
        let mut planner = VideoCandidatePlanner::default();
        planner.plan(request()).expect("first");
        let mut next = request();
        next.candidate_id = "candidate-1".into();
        planner.plan(next).expect("second");
        assert_eq!(planner.codec_gaps().len(), 1);
        assert_eq!(planner.codec_gaps()[0].occurrences, 2);
    }

    #[test]
    fn rejects_unobserved_drm_segmented_and_unapproved_candidates() {
        for (field, message) in [
            (
                "observed",
                "candidate was neither observed nor explicitly selected by the user",
            ),
            ("drm", "DRM-protected media is not supported"),
            (
                "segmented",
                "segmented media requires a proven site adapter",
            ),
            ("policy", "site policy or rights do not permit acquisition"),
        ] {
            let mut candidate = request();
            candidate.candidate_id = format!("candidate-{field}");
            match field {
                "observed" => candidate.observed_by_user = false,
                "drm" => candidate.drm_protected = true,
                "segmented" => candidate.segmented = true,
                "policy" => candidate.policy_allowed = false,
                _ => unreachable!(),
            }
            let mut planner = VideoCandidatePlanner::default();
            let plan = planner
                .plan(candidate)
                .expect("blocked plan is inspectable")
                .clone();
            assert_eq!(plan.state, CandidateState::PolicyBlocked);
            assert_eq!(plan.block_reason.as_deref(), Some(message));
            assert_eq!(
                planner.confirm(&plan.candidate_id),
                Err(VideoCandidateError::NotConfirmable)
            );
        }
    }
}
