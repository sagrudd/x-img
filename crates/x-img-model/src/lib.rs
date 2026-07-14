// SPDX-License-Identifier: MPL-2.0
//! Shared, storage-free product model boundaries.
//!
//! This crate intentionally contains no media payloads, source connectors, or
//! authentication material. Those integrations are introduced only after their
//! policy and contract gates are complete.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

/// The repository identity retained until the coordinated v1.0.0 rebrand.
pub const REPOSITORY_NAME: &str = "x-img";

/// A minimal product identity suitable for UI and host adapters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProductIdentity {
    /// The current repository and compatibility name.
    pub name: &'static str,
    /// The semantic version supplied by the workspace package authority.
    pub version: &'static str,
}

/// Returns the current build's public identity.
#[must_use]
pub const fn product_identity() -> ProductIdentity {
    ProductIdentity {
        name: REPOSITORY_NAME,
        version: env!("CARGO_PKG_VERSION"),
    }
}

/// The only configuration schema version currently accepted by the instance.
pub const INSTANCE_SCHEMA_VERSION: &str = "x-img.instance.v1";

/// A validated configuration document for one x-img instance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InstanceConfig {
    /// Versioned configuration envelope identifier.
    pub schema_version: String,
    /// Stable configuration identity.
    pub instance_id: String,
    /// Human-readable instance label.
    pub display_name: String,
    /// Opaque host context reference, never a session value.
    pub host_context_ref: HostManagedRef,
    /// Selected endpoint and logical ObjectStore identity.
    pub object_store_ref: ObjectStoreRef,
    /// Instance-level source defaults.
    pub defaults: Defaults,
    /// Explicit X account rules.
    pub x_accounts: Vec<XAccountConfig>,
    /// Explicit Instagram account rules.
    pub instagram_accounts: Vec<InstagramAccountConfig>,
    /// Explicit website origin rules.
    pub website_policies: Vec<WebsitePolicyConfig>,
}

/// Opaque reference to an authority-managed capability or context.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostManagedRef {
    /// Authority-owned reference kind.
    pub kind: HostManagedRefKind,
    /// Stable opaque authority identifier.
    pub id: String,
}

/// Supported authority-owned reference kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HostManagedRefKind {
    /// Monas provides the authenticated host context.
    #[serde(rename = "monas.host-context")]
    MonasHostContext,
    /// Monas retains a source connector authorization.
    #[serde(rename = "monas.connector-authorization")]
    MonasConnectorAuthorization,
    /// DASObjectStore provides an application capability.
    #[serde(rename = "dasobjectstore.application")]
    DasobjectstoreApplication,
}

/// Stable ObjectStore selection metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObjectStoreRef {
    /// Stable endpoint or appliance identifier.
    pub endpoint_id: String,
    /// Stable logical ObjectStore identifier.
    pub object_store_id: String,
    /// Managed destination prefix.
    pub prefix: String,
    /// Opaque DASObjectStore application reference.
    pub application_ref: HostManagedRef,
}

/// Shared source defaults.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Defaults {
    /// Default media eligibility policy.
    pub media_policy: MediaPolicy,
    /// Default bounded refresh budget.
    pub refresh_budget: RefreshBudget,
    /// Default review behavior.
    pub review_defaults: ReviewDefaults,
}

/// Media types and observation/open eligibility rules.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MediaPolicy {
    /// Whether images are eligible.
    pub images: bool,
    /// Whether videos are eligible.
    pub videos: bool,
    /// Whether animated images are eligible.
    pub animated_images: bool,
    /// Thumbnail acquisition eligibility.
    pub thumbnail_capture: ThumbnailCapture,
    /// Original acquisition eligibility.
    pub original_capture: OriginalCapture,
    /// Whether an allowed original may be retained.
    pub retain_original: bool,
}

/// Thumbnail capture eligibility.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThumbnailCapture {
    /// Do not capture thumbnails.
    Never,
    /// Capture only thumbnails actually observed by the user.
    ObservedOnly,
}

/// Original capture eligibility.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OriginalCapture {
    /// Do not capture originals.
    Never,
    /// Capture only after an explicit user open.
    ExplicitOpenOnly,
    /// Capture is separately policy-authorized.
    Allowed,
}

/// Explicit bounded refresh limits.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RefreshBudget {
    /// Maximum source requests in one refresh.
    pub max_requests: u64,
    /// Maximum pagination pages in one refresh.
    pub max_pages: u64,
    /// Maximum source items in one refresh.
    pub max_items: u64,
    /// Maximum bytes considered in one refresh.
    pub max_bytes: u64,
    /// Maximum refresh duration in seconds.
    pub max_duration_seconds: u64,
    /// Minimum seconds between refreshes.
    pub minimum_interval_seconds: u64,
}

/// Initial review presentation choices.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewDefaults {
    /// Initial review state for eligible committed records.
    pub initial_state: InitialReviewState,
    /// Whether a policy permits automatic review.
    pub auto_review: bool,
    /// Whether UI presentation groups records by source.
    pub group_by_source: bool,
}

/// Allowed initial review states.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InitialReviewState {
    /// Present newly committed media for review.
    New,
    /// Keep newly committed media hidden initially.
    Hidden,
}

/// X account configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct XAccountConfig {
    /// Per-record schema identifier.
    pub schema_version: String,
    /// Stable account-rule identifier.
    pub account_id: String,
    /// X handle without an at-sign.
    pub handle: String,
    /// Whether the account can be scheduled.
    pub enabled: bool,
    /// Account visibility and authorization mode.
    pub access_mode: XAccessMode,
    /// Opaque authorization reference when required.
    pub authorization_ref: Option<HostManagedRef>,
    /// Account media eligibility policy.
    pub media_policy: MediaPolicy,
    /// Account bounded refresh budget.
    pub refresh_budget: RefreshBudget,
    /// Account review defaults.
    pub review_defaults: ReviewDefaults,
}

/// X account access mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum XAccessMode {
    /// Public content under an approved connector policy.
    PublicOnly,
    /// User-authorized viewer access.
    AuthorizedViewer,
}

/// Instagram account configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InstagramAccountConfig {
    /// Per-record schema identifier.
    pub schema_version: String,
    /// Stable account-rule identifier.
    pub account_id: String,
    /// Instagram username without an at-sign.
    pub username: String,
    /// Whether the account can be scheduled.
    pub enabled: bool,
    /// Account visibility and authorization mode.
    pub account_class: InstagramAccountClass,
    /// Opaque authorization reference when required.
    pub authorization_ref: Option<HostManagedRef>,
    /// Account media eligibility policy.
    pub media_policy: MediaPolicy,
    /// Account bounded refresh budget.
    pub refresh_budget: RefreshBudget,
    /// Account review defaults.
    pub review_defaults: ReviewDefaults,
}

/// Instagram account class.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstagramAccountClass {
    /// Public account under an approved connector policy.
    Public,
    /// User-authorized viewer access.
    AuthorizedViewer,
}

/// Explicit Firefox website configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WebsitePolicyConfig {
    /// Per-record schema identifier.
    pub schema_version: String,
    /// Stable site-rule identifier.
    pub site_id: String,
    /// Exact HTTPS origin, with no path, wildcard, or credentials.
    pub origin: String,
    /// Whether the site is enabled.
    pub enabled: bool,
    /// Whether observed eligible media can be captured.
    pub capture_enabled: bool,
    /// Whether cached content can be substituted fail-open.
    pub substitution_enabled: bool,
    /// Site media eligibility policy.
    pub media_policy: MediaPolicy,
    /// Bounded observed-candidate limits.
    pub candidate_budget: CandidateBudget,
    /// Pinned adapter identity.
    pub adapter: WebsiteAdapter,
    /// Site review defaults.
    pub review_defaults: ReviewDefaults,
}

/// Bounded per-site candidate limits.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateBudget {
    /// Maximum candidates observed on one page.
    pub max_candidates_per_page: u64,
    /// Maximum bytes considered for one candidate.
    pub max_bytes_per_candidate: u64,
    /// Maximum candidates observed in one day.
    pub max_candidates_per_day: u64,
}

/// Versioned website adapter metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WebsiteAdapter {
    /// Adapter support class.
    pub kind: WebsiteAdapterKind,
    /// Adapter semantic version.
    pub version: String,
}

/// Supported website adapter classes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WebsiteAdapterKind {
    /// A named, reviewed adapter.
    Explicit,
    /// A bounded experimental adapter.
    ExperimentalGeneric,
}

/// Explains why an instance configuration was rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigValidationError {
    message: String,
}

impl ConfigValidationError {
    /// Creates an error with a stable, user-safe validation message.
    #[must_use]
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for ConfigValidationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for ConfigValidationError {}

impl InstanceConfig {
    /// Validates schema, identity, authority-reference, and source-rule invariants.
    ///
    /// This validates configuration only. It neither authenticates a reference
    /// nor schedules or acquires any source content.
    pub fn validate(&self) -> Result<(), ConfigValidationError> {
        require_exact(
            &self.schema_version,
            INSTANCE_SCHEMA_VERSION,
            "schema_version",
        )?;
        validate_identifier(&self.instance_id, "instance_id")?;
        validate_text(&self.display_name, 120, "display_name")?;
        validate_ref(
            &self.host_context_ref,
            HostManagedRefKind::MonasHostContext,
            "host_context_ref",
        )?;
        validate_identifier(
            &self.object_store_ref.endpoint_id,
            "object_store_ref.endpoint_id",
        )?;
        validate_identifier(
            &self.object_store_ref.object_store_id,
            "object_store_ref.object_store_id",
        )?;
        validate_prefix(&self.object_store_ref.prefix)?;
        validate_ref(
            &self.object_store_ref.application_ref,
            HostManagedRefKind::DasobjectstoreApplication,
            "object_store_ref.application_ref",
        )?;
        validate_refresh_budget(&self.defaults.refresh_budget, "defaults.refresh_budget")?;
        validate_unique(
            self.x_accounts
                .iter()
                .map(|account| account.account_id.as_str()),
            "X account_id",
        )?;
        validate_unique(
            self.x_accounts
                .iter()
                .map(|account| account.handle.to_ascii_lowercase()),
            "X handle",
        )?;
        validate_unique(
            self.instagram_accounts
                .iter()
                .map(|account| account.account_id.as_str()),
            "Instagram account_id",
        )?;
        validate_unique(
            self.instagram_accounts
                .iter()
                .map(|account| account.username.to_ascii_lowercase()),
            "Instagram username",
        )?;
        validate_unique(
            self.website_policies
                .iter()
                .map(|policy| policy.site_id.as_str()),
            "website site_id",
        )?;
        validate_unique(
            self.website_policies
                .iter()
                .map(|policy| policy.origin.as_str()),
            "website origin",
        )?;
        for account in &self.x_accounts {
            account.validate()?;
        }
        for account in &self.instagram_accounts {
            account.validate()?;
        }
        for policy in &self.website_policies {
            policy.validate()?;
        }
        Ok(())
    }
}

impl XAccountConfig {
    fn validate(&self) -> Result<(), ConfigValidationError> {
        require_exact(
            &self.schema_version,
            "x-img.x-account.v1",
            "X account schema_version",
        )?;
        validate_identifier(&self.account_id, "X account_id")?;
        validate_x_handle(&self.handle)?;
        validate_refresh_budget(&self.refresh_budget, "X refresh_budget")?;
        validate_optional_authorization(
            self.authorization_ref.as_ref(),
            self.enabled || self.access_mode == XAccessMode::AuthorizedViewer,
            "X authorization_ref",
        )
    }
}

impl InstagramAccountConfig {
    fn validate(&self) -> Result<(), ConfigValidationError> {
        require_exact(
            &self.schema_version,
            "x-img.instagram-account.v1",
            "Instagram account schema_version",
        )?;
        validate_identifier(&self.account_id, "Instagram account_id")?;
        validate_instagram_username(&self.username)?;
        validate_refresh_budget(&self.refresh_budget, "Instagram refresh_budget")?;
        validate_optional_authorization(
            self.authorization_ref.as_ref(),
            self.enabled || self.account_class == InstagramAccountClass::AuthorizedViewer,
            "Instagram authorization_ref",
        )
    }
}

impl WebsitePolicyConfig {
    fn validate(&self) -> Result<(), ConfigValidationError> {
        require_exact(
            &self.schema_version,
            "x-img.website-policy.v1",
            "website schema_version",
        )?;
        validate_identifier(&self.site_id, "website site_id")?;
        validate_https_origin(&self.origin)?;
        validate_semver(&self.adapter.version, "website adapter version")
    }
}

fn require_exact(value: &str, expected: &str, field: &str) -> Result<(), ConfigValidationError> {
    if value == expected {
        Ok(())
    } else {
        Err(ConfigValidationError::new(format!(
            "{field} must be {expected:?}; future or incompatible schema versions are rejected"
        )))
    }
}

fn validate_ref(
    reference: &HostManagedRef,
    expected_kind: HostManagedRefKind,
    field: &str,
) -> Result<(), ConfigValidationError> {
    if reference.kind != expected_kind {
        return Err(ConfigValidationError::new(format!(
            "{field} has an incompatible reference kind"
        )));
    }
    validate_authority_id(&reference.id, field)
}

fn validate_optional_authorization(
    reference: Option<&HostManagedRef>,
    required: bool,
    field: &str,
) -> Result<(), ConfigValidationError> {
    match (required, reference) {
        (true, None) => Err(ConfigValidationError::new(format!(
            "{field} is required for an enabled or authorized account"
        ))),
        (_, Some(reference)) => validate_ref(
            reference,
            HostManagedRefKind::MonasConnectorAuthorization,
            field,
        ),
        (false, None) => Ok(()),
    }
}

fn validate_unique<'a>(
    values: impl IntoIterator<Item = impl AsRef<str> + 'a>,
    field: &str,
) -> Result<(), ConfigValidationError> {
    let mut seen = HashSet::new();
    for value in values {
        if !seen.insert(value.as_ref().to_owned()) {
            return Err(ConfigValidationError::new(format!(
                "duplicate {field} is not permitted"
            )));
        }
    }
    Ok(())
}

fn validate_identifier(value: &str, field: &str) -> Result<(), ConfigValidationError> {
    let allowed = |character: char| {
        character.is_ascii_lowercase() || character.is_ascii_digit() || character == '-'
    };
    if value.is_empty()
        || value.len() > 64
        || !value.chars().all(allowed)
        || !value.as_bytes()[0].is_ascii_alphanumeric()
    {
        return Err(ConfigValidationError::new(format!(
            "{field} must be a 1–64 character lowercase identifier"
        )));
    }
    Ok(())
}

fn validate_authority_id(value: &str, field: &str) -> Result<(), ConfigValidationError> {
    let allowed = |character: char| {
        character.is_ascii_lowercase()
            || character.is_ascii_digit()
            || matches!(character, '.' | '_' | ':' | '-')
    };
    if value.is_empty()
        || value.len() > 128
        || !value.chars().all(allowed)
        || !value.as_bytes()[0].is_ascii_alphanumeric()
    {
        return Err(ConfigValidationError::new(format!(
            "{field} must contain a stable opaque authority identifier"
        )));
    }
    Ok(())
}

fn validate_text(value: &str, maximum: usize, field: &str) -> Result<(), ConfigValidationError> {
    if value.is_empty() || value.len() > maximum {
        return Err(ConfigValidationError::new(format!(
            "{field} must contain 1–{maximum} characters"
        )));
    }
    Ok(())
}

fn validate_prefix(value: &str) -> Result<(), ConfigValidationError> {
    if value.is_empty()
        || value.len() > 256
        || value.starts_with('/')
        || !value.ends_with('/')
        || value.contains('\n')
    {
        return Err(ConfigValidationError::new(
            "object_store_ref.prefix must be a non-root managed prefix ending in '/'",
        ));
    }
    Ok(())
}

fn validate_x_handle(value: &str) -> Result<(), ConfigValidationError> {
    if value.is_empty()
        || value.len() > 15
        || !value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '_')
    {
        return Err(ConfigValidationError::new(
            "X handle must be 1–15 ASCII letters, digits, or underscores without '@'",
        ));
    }
    Ok(())
}

fn validate_instagram_username(value: &str) -> Result<(), ConfigValidationError> {
    if value.is_empty()
        || value.len() > 30
        || !value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '.' | '_'))
    {
        return Err(ConfigValidationError::new(
            "Instagram username must be 1–30 ASCII letters, digits, periods, or underscores without '@'",
        ));
    }
    Ok(())
}

fn validate_https_origin(value: &str) -> Result<(), ConfigValidationError> {
    let Some(host) = value.strip_prefix("https://") else {
        return Err(ConfigValidationError::new("website origin must use HTTPS"));
    };
    if host.is_empty()
        || host.contains(['/', '?', '#', '@', '*', ' '])
        || host
            .split_once(':')
            .is_some_and(|(_, port)| port.is_empty() || port.parse::<u16>().is_err())
    {
        return Err(ConfigValidationError::new(
            "website origin must be one exact HTTPS origin without path, credentials, or wildcards",
        ));
    }
    Ok(())
}

fn validate_semver(value: &str, field: &str) -> Result<(), ConfigValidationError> {
    let valid = value.split('.').count() == 3
        && value.split('.').all(|part| {
            !part.is_empty() && part.chars().all(|character| character.is_ascii_digit())
        });
    if !valid {
        return Err(ConfigValidationError::new(format!(
            "{field} must use a three-part numeric semantic version"
        )));
    }
    Ok(())
}

fn validate_refresh_budget(
    budget: &RefreshBudget,
    field: &str,
) -> Result<(), ConfigValidationError> {
    if budget.max_duration_seconds == 0 {
        return Err(ConfigValidationError::new(format!(
            "{field}.max_duration_seconds must be at least one second"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{REPOSITORY_NAME, product_identity};

    #[test]
    fn identity_uses_the_current_repository_name() {
        assert_eq!(product_identity().name, REPOSITORY_NAME);
    }
}
