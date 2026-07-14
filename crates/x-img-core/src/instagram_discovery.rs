// SPDX-License-Identifier: MPL-2.0
//! Fixture-driven incremental Instagram media-discovery planning.
//!
//! This module accepts metadata pages only. It contains no Meta endpoint,
//! browser automation, token, cookie, media byte, or storage operation.

#![allow(missing_docs)]

use std::collections::BTreeMap;

use serde::Deserialize;

use crate::reconciliation::ReconciliationRequest;

pub const INSTAGRAM_DISCOVERY_FIXTURE_SCHEMA: &str = "x-img.instagram-discovery-fixtures.v1";
pub const INSTAGRAM_DISCOVERY_ADAPTER_VERSION: &str = "x-img.instagram-discovery.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstagramItemKind {
    Post,
    Carousel,
    Reel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstagramMediaKind {
    Image,
    Video,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstagramCredentialState {
    FixtureActive,
    Expired,
    Revoked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstagramTokenOutcome {
    FixtureOnlyEligible,
    ReauthorizationRequired,
}

/// Evaluates opaque host-token lifecycle state without accepting a token value.
#[must_use]
pub const fn evaluate_token_lifecycle(state: InstagramCredentialState) -> InstagramTokenOutcome {
    match state {
        InstagramCredentialState::FixtureActive => InstagramTokenOutcome::FixtureOnlyEligible,
        InstagramCredentialState::Expired | InstagramCredentialState::Revoked => {
            InstagramTokenOutcome::ReauthorizationRequired
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct InstagramVariant {
    pub variant_id: String,
    pub content_type: String,
    pub width: u32,
    pub height: u32,
    pub bitrate: u64,
    pub source_url_alias: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct InstagramMedia {
    pub media_id: String,
    pub kind: InstagramMediaKind,
    pub expected_checksum_sha256: String,
    pub variants: Vec<InstagramVariant>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct InstagramItem {
    pub item_id: String,
    pub kind: InstagramItemKind,
    pub media: Vec<InstagramMedia>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct InstagramDiscoveryPage {
    pub request_cursor: Option<String>,
    pub next_cursor: Option<String>,
    pub items: Vec<InstagramItem>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InstagramDiscoveryBudget {
    pub max_pages: u32,
    pub max_items: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstagramMediaCandidate {
    pub account_id: String,
    pub item_id: String,
    pub item_kind: InstagramItemKind,
    pub media_id: String,
    pub media_kind: InstagramMediaKind,
    pub canonical_media_identity: String,
    pub selected_variant: InstagramVariant,
    pub expected_checksum_sha256: String,
    pub discovery_time_unix_seconds: u64,
    pub adapter_version: String,
    pub policy_result: String,
}

impl InstagramMediaCandidate {
    /// Converts discovered metadata into the existing immutable reconciliation key input.
    pub fn reconciliation_request(&self) -> Result<ReconciliationRequest, InstagramDiscoveryError> {
        ReconciliationRequest::new(
            self.canonical_media_identity.clone(),
            self.expected_checksum_sha256.clone(),
            [self.selected_variant.source_url_alias.clone()],
        )
        .map_err(|_| InstagramDiscoveryError::InvalidMetadata("reconciliation_request"))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstagramDiscoveryResult {
    pub candidates: Vec<InstagramMediaCandidate>,
    pub next_cursor: Option<String>,
    pub truncated_by_budget: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstagramDiscoveryError {
    InvalidMetadata(&'static str),
    InvalidCursorChain,
    ReauthorizationRequired,
    UnsupportedItemShape,
}
impl std::fmt::Display for InstagramDiscoveryError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "Instagram discovery rejected: {self:?}")
    }
}
impl std::error::Error for InstagramDiscoveryError {}

/// Plans metadata-only pages from a future approved adapter in fixture mode.
pub fn plan_incremental_discovery(
    account_id: &str,
    credential_state: InstagramCredentialState,
    pages: &[InstagramDiscoveryPage],
    budget: InstagramDiscoveryBudget,
    discovery_time_unix_seconds: u64,
) -> Result<InstagramDiscoveryResult, InstagramDiscoveryError> {
    if evaluate_token_lifecycle(credential_state) == InstagramTokenOutcome::ReauthorizationRequired
    {
        return Err(InstagramDiscoveryError::ReauthorizationRequired);
    }
    if !is_identifier(account_id) || budget.max_pages == 0 || budget.max_items == 0 {
        return Err(InstagramDiscoveryError::InvalidMetadata(
            "account_or_budget",
        ));
    }
    let mut candidates = BTreeMap::new();
    let mut expected_cursor = None;
    let mut next_cursor = None;
    let mut truncated_by_budget = false;
    for (page_index, page) in pages.iter().enumerate() {
        if u32::try_from(page_index).expect("usize exceeds u32 only on impossible input")
            >= budget.max_pages
        {
            truncated_by_budget = true;
            break;
        }
        if page.request_cursor != expected_cursor {
            return Err(InstagramDiscoveryError::InvalidCursorChain);
        }
        for item in &page.items {
            validate_item(item)?;
            for media in &item.media {
                let candidate =
                    candidate_from_media(account_id, item, media, discovery_time_unix_seconds)?;
                if candidates.contains_key(&candidate.canonical_media_identity) {
                    continue;
                }
                if u32::try_from(candidates.len())
                    .expect("usize exceeds u32 only on impossible input")
                    >= budget.max_items
                {
                    truncated_by_budget = true;
                    break;
                }
                candidates.insert(candidate.canonical_media_identity.clone(), candidate);
            }
            if truncated_by_budget {
                break;
            }
        }
        next_cursor = page.next_cursor.clone();
        expected_cursor = page.next_cursor.clone();
        if truncated_by_budget || page.next_cursor.is_none() {
            break;
        }
    }
    Ok(InstagramDiscoveryResult {
        candidates: candidates.into_values().collect(),
        next_cursor,
        truncated_by_budget,
    })
}

fn validate_item(item: &InstagramItem) -> Result<(), InstagramDiscoveryError> {
    if !is_identifier(&item.item_id) || item.media.is_empty() {
        return Err(InstagramDiscoveryError::InvalidMetadata("item"));
    }
    match item.kind {
        InstagramItemKind::Post | InstagramItemKind::Reel if item.media.len() != 1 => {
            Err(InstagramDiscoveryError::UnsupportedItemShape)
        }
        InstagramItemKind::Carousel if item.media.len() < 2 => {
            Err(InstagramDiscoveryError::UnsupportedItemShape)
        }
        _ => Ok(()),
    }
}

fn candidate_from_media(
    account_id: &str,
    item: &InstagramItem,
    media: &InstagramMedia,
    discovery_time_unix_seconds: u64,
) -> Result<InstagramMediaCandidate, InstagramDiscoveryError> {
    if !is_identifier(&media.media_id) || !is_sha256(&media.expected_checksum_sha256) {
        return Err(InstagramDiscoveryError::InvalidMetadata(
            "media_identity_or_checksum",
        ));
    }
    let selected_variant = media
        .variants
        .iter()
        .filter(|variant| is_supported(media.kind, variant))
        .max_by_key(|variant| {
            (
                u64::from(variant.width) * u64::from(variant.height),
                variant.bitrate,
                variant.variant_id.as_str(),
            )
        })
        .cloned()
        .ok_or(InstagramDiscoveryError::InvalidMetadata(
            "supported_variant",
        ))?;
    if !is_identifier(&selected_variant.variant_id)
        || !is_safe_alias(&selected_variant.source_url_alias)
    {
        return Err(InstagramDiscoveryError::InvalidMetadata("variant"));
    }
    Ok(InstagramMediaCandidate {
        account_id: account_id.to_owned(),
        item_id: item.item_id.clone(),
        item_kind: item.kind,
        media_id: media.media_id.clone(),
        media_kind: media.kind,
        canonical_media_identity: format!(
            "instagram:{account_id}:{}:{}",
            item.item_id, media.media_id
        ),
        selected_variant,
        expected_checksum_sha256: media.expected_checksum_sha256.clone(),
        discovery_time_unix_seconds,
        adapter_version: INSTAGRAM_DISCOVERY_ADAPTER_VERSION.to_owned(),
        policy_result: "fixture-only-instagram-product-gate-open".to_owned(),
    })
}

fn is_supported(kind: InstagramMediaKind, variant: &InstagramVariant) -> bool {
    match kind {
        InstagramMediaKind::Image => matches!(
            variant.content_type.as_str(),
            "image/jpeg" | "image/png" | "image/webp"
        ),
        InstagramMediaKind::Video => variant.content_type == "video/mp4",
    }
}
fn is_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.as_bytes()[0].is_ascii_lowercase()
        && value.chars().all(|character| {
            character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || matches!(character, '.' | '_' | ':' | '-')
        })
}
fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .chars()
            .all(|character| character.is_ascii_digit() || matches!(character, 'a'..='f'))
}
fn is_safe_alias(value: &str) -> bool {
    value.starts_with("https://")
        && value.len() <= 2048
        && !value.contains(['@', '?', '#', ' ', '\n', '\r'])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        acquisition::VerifiedObject,
        reconciliation::{AuthorityObservation, ReconciliationCatalogue, ReconciliationOutcome},
    };
    use std::collections::BTreeSet;

    #[derive(Deserialize)]
    struct Fixture {
        schema_version: String,
        account_id: String,
        credential_state: InstagramCredentialState,
        budget: FixtureBudget,
        expected_candidate_identities: BTreeSet<String>,
        pages: Vec<InstagramDiscoveryPage>,
    }
    #[derive(Deserialize)]
    struct FixtureBudget {
        max_pages: u32,
        max_items: u32,
    }
    fn fixture() -> Fixture {
        serde_json::from_slice(include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/instagram-discovery/v1/pages.json"
        )))
        .expect("fixture")
    }

    #[test]
    fn fixture_supports_posts_carousels_videos_pagination_and_provenance() {
        let fixture = fixture();
        assert_eq!(fixture.schema_version, INSTAGRAM_DISCOVERY_FIXTURE_SCHEMA);
        let result = plan_incremental_discovery(
            &fixture.account_id,
            fixture.credential_state,
            &fixture.pages,
            InstagramDiscoveryBudget {
                max_pages: fixture.budget.max_pages,
                max_items: fixture.budget.max_items,
            },
            1_784_000_000,
        )
        .expect("plan");
        assert_eq!(
            result
                .candidates
                .iter()
                .map(|candidate| candidate.canonical_media_identity.clone())
                .collect::<BTreeSet<_>>(),
            fixture.expected_candidate_identities
        );
        assert!(result.candidates.iter().any(|candidate| candidate.item_kind
            == InstagramItemKind::Carousel
            && candidate.media_kind == InstagramMediaKind::Video
            && candidate.selected_variant.variant_id == "carousel-video-large"));
        assert!(
            result
                .candidates
                .iter()
                .any(|candidate| candidate.item_kind == InstagramItemKind::Reel
                    && candidate.selected_variant.variant_id == "reel-mp4")
        );
        assert!(
            result
                .candidates
                .iter()
                .all(|candidate| candidate.policy_result
                    == "fixture-only-instagram-product-gate-open")
        );
    }

    #[test]
    fn expired_or_revoked_host_credential_requires_reauthorization_without_a_token() {
        let fixture = fixture();
        for state in [
            InstagramCredentialState::Expired,
            InstagramCredentialState::Revoked,
        ] {
            assert_eq!(
                evaluate_token_lifecycle(state),
                InstagramTokenOutcome::ReauthorizationRequired
            );
            assert_eq!(
                plan_incremental_discovery(
                    &fixture.account_id,
                    state,
                    &fixture.pages,
                    InstagramDiscoveryBudget {
                        max_pages: 2,
                        max_items: 8
                    },
                    1
                ),
                Err(InstagramDiscoveryError::ReauthorizationRequired)
            );
        }
    }

    #[test]
    fn budget_cursor_and_idempotency_are_bounded() {
        let fixture = fixture();
        let first_page = plan_incremental_discovery(
            &fixture.account_id,
            fixture.credential_state,
            &fixture.pages,
            InstagramDiscoveryBudget {
                max_pages: 1,
                max_items: 8,
            },
            1,
        )
        .expect("first page");
        assert!(first_page.truncated_by_budget);
        assert_eq!(first_page.next_cursor.as_deref(), Some("ig-cursor-2"));
        let candidate = plan_incremental_discovery(
            &fixture.account_id,
            fixture.credential_state,
            &fixture.pages,
            InstagramDiscoveryBudget {
                max_pages: 2,
                max_items: 8,
            },
            1,
        )
        .expect("full plan")
        .candidates
        .into_iter()
        .next()
        .expect("candidate");
        let request = candidate.reconciliation_request().expect("request");
        let object = VerifiedObject::new(
            "fixture-endpoint",
            "fixture-store",
            "fixture-object",
            candidate.expected_checksum_sha256.clone(),
        )
        .expect("object");
        let mut catalogue = ReconciliationCatalogue::default();
        assert!(matches!(
            catalogue
                .reconcile(
                    request.clone(),
                    AuthorityObservation::Verified(object.clone())
                )
                .expect("first"),
            ReconciliationOutcome::Committed { .. }
        ));
        assert!(matches!(
            catalogue
                .reconcile(request, AuthorityObservation::Verified(object))
                .expect("replay"),
            ReconciliationOutcome::AlreadyCommitted { .. }
        ));
        assert_eq!(catalogue.committed_len(), 1);
    }
}
