// SPDX-License-Identifier: MPL-2.0
//! Explicit, review-before-save selection of X accounts followed by the viewer.
//!
//! A future official API adapter supplies candidates. This module does not call
//! X, enable every candidate, or write configuration on its own.

#![allow(missing_docs)]

use crate::x_oauth::{XTokenGrant, authorizes_viewing_account};
use std::collections::BTreeSet;
use x_img_model::{HostManagedRef, InstanceConfig, XAccessMode, XAccountConfig};

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
pub struct FollowedXAccount {
    pub x_user_id: String,
    pub handle: String,
    pub requires_viewer_authorization: bool,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XImportDiff {
    pub added: Vec<String>,
    pub already_configured: Vec<String>,
    pub not_selected: Vec<String>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XImportPreview {
    pub candidates: Vec<FollowedXAccount>,
    pub diff: XImportDiff,
    candidate_config: InstanceConfig,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum XFollowedImportError {
    UnauthorizedViewer,
    InvalidCandidate(String),
    UnknownSelection(String),
    DuplicateSelection,
    Config(String),
    Unconfirmed,
}
impl std::fmt::Display for XFollowedImportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnauthorizedViewer => {
                f.write_str("X followed-account import is not authorized for this viewing account")
            }
            Self::InvalidCandidate(value) => {
                write!(f, "invalid followed X account candidate: {value}")
            }
            Self::UnknownSelection(value) => write!(
                f,
                "selected X account was not returned by the authorized follow list: {value}"
            ),
            Self::DuplicateSelection => {
                f.write_str("the same followed X account was selected more than once")
            }
            Self::Config(value) => write!(f, "imported X configuration is invalid: {value}"),
            Self::Unconfirmed => {
                f.write_str("reviewed X account import requires explicit confirmation")
            }
        }
    }
}
impl std::error::Error for XFollowedImportError {}

/// Produces task-pane data and a candidate configuration without writing it.
pub fn preview_import(
    config: &InstanceConfig,
    grant: &XTokenGrant,
    viewing_x_user_id: &str,
    candidates: Vec<FollowedXAccount>,
    selected_x_user_ids: &[String],
    authorization_ref: HostManagedRef,
    now_unix_seconds: u64,
) -> Result<XImportPreview, XFollowedImportError> {
    if !authorizes_viewing_account(grant, viewing_x_user_id, now_unix_seconds) {
        return Err(XFollowedImportError::UnauthorizedViewer);
    }
    if authorization_ref.kind != x_img_model::HostManagedRefKind::MonasConnectorAuthorization {
        return Err(XFollowedImportError::Config(
            "import authorization must be a Monas connector reference".to_owned(),
        ));
    }
    let mut returned_ids = BTreeSet::new();
    let mut returned_handles = BTreeSet::new();
    for candidate in &candidates {
        validate_candidate(candidate)?;
        if !returned_ids.insert(candidate.x_user_id.clone())
            || !returned_handles.insert(candidate.handle.to_ascii_lowercase())
        {
            return Err(XFollowedImportError::InvalidCandidate(
                "duplicate user ID or handle".to_owned(),
            ));
        }
    }
    let mut selected = BTreeSet::new();
    for id in selected_x_user_ids {
        if !selected.insert(id.clone()) {
            return Err(XFollowedImportError::DuplicateSelection);
        }
        if !returned_ids.contains(id) {
            return Err(XFollowedImportError::UnknownSelection(id.clone()));
        }
    }
    let existing_handles: BTreeSet<String> = config
        .x_accounts
        .iter()
        .map(|account| account.handle.to_ascii_lowercase())
        .collect();
    let existing_account_ids: BTreeSet<&str> = config
        .x_accounts
        .iter()
        .map(|account| account.account_id.as_str())
        .collect();
    let mut candidate_config = config.clone();
    let mut added = Vec::new();
    let mut already_configured = Vec::new();
    let mut not_selected = Vec::new();
    for candidate in &candidates {
        if !selected.contains(&candidate.x_user_id) {
            not_selected.push(candidate.handle.clone());
            continue;
        }
        if existing_handles.contains(&candidate.handle.to_ascii_lowercase())
            || existing_account_ids.contains(format!("x-{}", candidate.x_user_id).as_str())
        {
            already_configured.push(candidate.handle.clone());
            continue;
        }
        candidate_config.x_accounts.push(XAccountConfig {
            schema_version: "x-img.x-account.v1".to_owned(),
            account_id: format!("x-{}", candidate.x_user_id),
            handle: candidate.handle.clone(),
            enabled: true,
            access_mode: if candidate.requires_viewer_authorization {
                XAccessMode::AuthorizedViewer
            } else {
                XAccessMode::PublicOnly
            },
            authorization_ref: Some(authorization_ref.clone()),
            media_policy: candidate_config.defaults.media_policy.clone(),
            refresh_budget: candidate_config.defaults.refresh_budget.clone(),
            review_defaults: candidate_config.defaults.review_defaults.clone(),
        });
        added.push(candidate.handle.clone());
    }
    candidate_config
        .validate()
        .map_err(|error| XFollowedImportError::Config(error.to_string()))?;
    Ok(XImportPreview {
        candidates,
        diff: XImportDiff {
            added,
            already_configured,
            not_selected,
        },
        candidate_config,
    })
}

/// Returns the reviewed configuration only after an explicit confirmation click.
pub fn confirm_import(
    preview: XImportPreview,
    confirmed: bool,
) -> Result<InstanceConfig, XFollowedImportError> {
    if !confirmed {
        return Err(XFollowedImportError::Unconfirmed);
    }
    Ok(preview.candidate_config)
}
fn validate_candidate(candidate: &FollowedXAccount) -> Result<(), XFollowedImportError> {
    if candidate.x_user_id.is_empty()
        || candidate.x_user_id.len() > 64
        || !candidate
            .x_user_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    {
        return Err(XFollowedImportError::InvalidCandidate(
            "X user ID must be a stable identifier".to_owned(),
        ));
    }
    if candidate.handle.is_empty()
        || candidate.handle.len() > 15
        || !candidate
            .handle
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
    {
        return Err(XFollowedImportError::InvalidCandidate(
            "handle must be a 1-15 character X handle without @".to_owned(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;
    fn config() -> InstanceConfig {
        serde_json::from_slice(include_bytes!("../../../examples/config/instance.v1.json"))
            .expect("config")
    }
    fn grant() -> XTokenGrant {
        XTokenGrant {
            credential_ref: "monas.x-oauth:fixture".into(),
            host_actor_ref: "monas.host-context:fixture-user".into(),
            viewing_x_user_id: "viewer-1".into(),
            scopes: ["tweet.read", "users.read", "follows.read", "offline.access"]
                .into_iter()
                .map(str::to_owned)
                .collect::<BTreeSet<_>>(),
            expires_at_unix_seconds: 1000,
        }
    }
    fn authorization() -> HostManagedRef {
        HostManagedRef {
            kind: x_img_model::HostManagedRefKind::MonasConnectorAuthorization,
            id: "fixture-x-import".into(),
        }
    }
    #[test]
    fn preview_is_explicit_and_diff_is_reviewable_before_confirmed_save() {
        let candidates = vec![
            FollowedXAccount {
                x_user_id: "12345".into(),
                handle: "SelectedArtist".into(),
                requires_viewer_authorization: true,
            },
            FollowedXAccount {
                x_user_id: "67890".into(),
                handle: "NotSelected".into(),
                requires_viewer_authorization: false,
            },
        ];
        let preview = preview_import(
            &config(),
            &grant(),
            "viewer-1",
            candidates,
            &["12345".into()],
            authorization(),
            500,
        )
        .expect("preview");
        assert_eq!(preview.diff.added, ["SelectedArtist"]);
        assert_eq!(preview.diff.not_selected, ["NotSelected"]);
        assert_eq!(
            confirm_import(preview.clone(), false),
            Err(XFollowedImportError::Unconfirmed)
        );
        let saved = confirm_import(preview, true).expect("confirmed config");
        assert!(
            saved
                .x_accounts
                .iter()
                .any(|account| account.handle == "SelectedArtist"
                    && account.access_mode == XAccessMode::AuthorizedViewer)
        );
    }
    #[test]
    fn unknown_or_unauthorized_candidates_never_enter_allowlist() {
        let candidate = FollowedXAccount {
            x_user_id: "12345".into(),
            handle: "SelectedArtist".into(),
            requires_viewer_authorization: false,
        };
        assert!(matches!(
            preview_import(
                &config(),
                &grant(),
                "other",
                vec![candidate.clone()],
                &["12345".into()],
                authorization(),
                500
            ),
            Err(XFollowedImportError::UnauthorizedViewer)
        ));
        assert!(matches!(
            preview_import(
                &config(),
                &grant(),
                "viewer-1",
                vec![candidate],
                &["missing".into()],
                authorization(),
                500
            ),
            Err(XFollowedImportError::UnknownSelection(_))
        ));
    }
}
