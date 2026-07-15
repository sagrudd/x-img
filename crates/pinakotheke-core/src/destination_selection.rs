// SPDX-License-Identifier: MPL-2.0
//! Explicit endpoint/ObjectStore review and pre-commit revalidation.
//!
//! This module holds no credential and cannot discover remotely. A host adapter
//! supplies the authority-visible inventory and current state.

#![allow(missing_docs)]

use crate::destination::{DestinationError, ReviewedDestination, validate_reviewed_destination};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DestinationRow {
    pub endpoint_id: String,
    pub endpoint_label: String,
    pub object_store_id: String,
    pub object_store_label: String,
    pub status_word: String,
    pub writable: bool,
    pub quota_available_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DestinationSnapshot {
    pub endpoint_id: String,
    pub object_store_id: String,
    pub endpoint_label: String,
    pub object_store_label: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
pub struct DestinationAuthorityState {
    pub endpoint_id: String,
    pub object_store_id: String,
    pub endpoint_label: String,
    pub object_store_label: String,
    pub endpoint_present: bool,
    pub object_store_present: bool,
    pub tls_trusted: bool,
    pub paired: bool,
    pub pairing_expires_at_unix_seconds: u64,
    pub ready: bool,
    pub writable: bool,
    pub quota_available_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DestinationRevalidationError {
    RemovedEndpoint,
    RemovedObjectStore,
    ChangedEndpoint,
    ChangedObjectStore,
    RenamedEndpoint,
    RenamedObjectStore,
    TlsNotTrusted,
    PairingExpired,
    NeedsReconnect,
    ReadOnly,
    OverQuota,
}

impl std::fmt::Display for DestinationRevalidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::RemovedEndpoint => "reviewed endpoint was removed",
            Self::RemovedObjectStore => "reviewed ObjectStore was removed",
            Self::ChangedEndpoint => "authority state names another endpoint",
            Self::ChangedObjectStore => "authority state names another ObjectStore",
            Self::RenamedEndpoint => "reviewed endpoint label changed; review again",
            Self::RenamedObjectStore => "reviewed ObjectStore label changed; review again",
            Self::TlsNotTrusted => "endpoint TLS is not trusted",
            Self::PairingExpired => "endpoint pairing has expired",
            Self::NeedsReconnect => "endpoint needs reconnect",
            Self::ReadOnly => "ObjectStore is read-only",
            Self::OverQuota => "ObjectStore has no remaining quota",
        })
    }
}

impl std::error::Error for DestinationRevalidationError {}

/// Returns an accessibility-ready row for the reviewed target. The task pane
/// must render `status_word` in text and show endpoint/store labels together.
pub fn visible_destination_rows(inventory: &[u8]) -> Result<Vec<DestinationRow>, DestinationError> {
    validate_reviewed_destination(inventory)?;
    let document: serde_json::Value = serde_json::from_slice(inventory)
        .map_err(|error| DestinationError::Json(error.to_string()))?;
    let mut rows = Vec::new();
    for endpoint in document["endpoints"]
        .as_array()
        .expect("validated endpoints")
    {
        for store in endpoint["object_stores"]
            .as_array()
            .expect("validated stores")
        {
            let status_word = match (
                endpoint["health"].as_str(),
                store["health"].as_str(),
                store["writable"].as_bool(),
            ) {
                (Some("needs_reconnect"), _, _) => "Needs reconnect",
                (Some("unavailable"), _, _) | (_, Some("unavailable"), _) => "Unavailable",
                (_, Some("read_only"), _) | (_, _, Some(false)) => "Read-only",
                _ => "Ready",
            };
            rows.push(DestinationRow {
                endpoint_id: endpoint["endpoint_id"].as_str().expect("ID").to_owned(),
                endpoint_label: endpoint["display_name"].as_str().expect("label").to_owned(),
                object_store_id: store["object_store_id"].as_str().expect("ID").to_owned(),
                object_store_label: store["display_name"].as_str().expect("label").to_owned(),
                status_word: status_word.to_owned(),
                writable: store["writable"].as_bool().expect("validated writable"),
                quota_available_bytes: store["quota_available_bytes"]
                    .as_u64()
                    .expect("validated quota"),
            });
        }
    }
    Ok(rows)
}

pub fn reviewed_row(
    inventory: &[u8],
) -> Result<(ReviewedDestination, DestinationRow), DestinationError> {
    let reviewed = validate_reviewed_destination(inventory)?;
    let document: serde_json::Value = serde_json::from_slice(inventory)
        .map_err(|error| DestinationError::Json(error.to_string()))?;
    let endpoints = document["endpoints"]
        .as_array()
        .expect("validated endpoints");
    let endpoint = endpoints
        .iter()
        .find(|item| item["endpoint_id"] == reviewed.endpoint_id)
        .expect("validated endpoint");
    let store = endpoint["object_stores"]
        .as_array()
        .expect("validated stores")
        .iter()
        .find(|item| item["object_store_id"] == reviewed.object_store_id)
        .expect("validated store");
    Ok((
        reviewed,
        DestinationRow {
            endpoint_id: endpoint["endpoint_id"].as_str().expect("ID").to_owned(),
            endpoint_label: endpoint["display_name"].as_str().expect("label").to_owned(),
            object_store_id: store["object_store_id"].as_str().expect("ID").to_owned(),
            object_store_label: store["display_name"].as_str().expect("label").to_owned(),
            status_word: if store["writable"].as_bool() == Some(true) {
                "Ready".to_owned()
            } else {
                "Read-only".to_owned()
            },
            writable: store["writable"].as_bool().expect("validated writable"),
            quota_available_bytes: store["quota_available_bytes"]
                .as_u64()
                .expect("validated quota"),
        },
    ))
}

/// Revalidates exactly the pair the user reviewed. It never picks a fallback,
/// including when another endpoint exposes an ObjectStore with the same alias.
pub fn revalidate_before_commit(
    snapshot: &DestinationSnapshot,
    current: &DestinationAuthorityState,
    now_unix_seconds: u64,
) -> Result<(), DestinationRevalidationError> {
    if !current.endpoint_present {
        return Err(DestinationRevalidationError::RemovedEndpoint);
    }
    if current.endpoint_id != snapshot.endpoint_id {
        return Err(DestinationRevalidationError::ChangedEndpoint);
    }
    if current.endpoint_label != snapshot.endpoint_label {
        return Err(DestinationRevalidationError::RenamedEndpoint);
    }
    if !current.object_store_present {
        return Err(DestinationRevalidationError::RemovedObjectStore);
    }
    if current.object_store_id != snapshot.object_store_id {
        return Err(DestinationRevalidationError::ChangedObjectStore);
    }
    if current.object_store_label != snapshot.object_store_label {
        return Err(DestinationRevalidationError::RenamedObjectStore);
    }
    if !current.tls_trusted {
        return Err(DestinationRevalidationError::TlsNotTrusted);
    }
    if !current.paired || now_unix_seconds >= current.pairing_expires_at_unix_seconds {
        return Err(DestinationRevalidationError::PairingExpired);
    }
    if !current.ready {
        return Err(DestinationRevalidationError::NeedsReconnect);
    }
    if !current.writable {
        return Err(DestinationRevalidationError::ReadOnly);
    }
    if current.quota_available_bytes == 0 {
        return Err(DestinationRevalidationError::OverQuota);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn snapshot() -> DestinationSnapshot {
        DestinationSnapshot {
            endpoint_id: "remote-appliance-1".to_owned(),
            object_store_id: "remote-archive".to_owned(),
            endpoint_label: "Studio appliance".to_owned(),
            object_store_label: "Archive".to_owned(),
        }
    }
    fn state() -> DestinationAuthorityState {
        DestinationAuthorityState {
            endpoint_id: "remote-appliance-1".to_owned(),
            object_store_id: "remote-archive".to_owned(),
            endpoint_label: "Studio appliance".to_owned(),
            object_store_label: "Archive".to_owned(),
            endpoint_present: true,
            object_store_present: true,
            tls_trusted: true,
            paired: true,
            pairing_expires_at_unix_seconds: 1000,
            ready: true,
            writable: true,
            quota_available_bytes: 1,
        }
    }

    #[test]
    fn exposes_a_reviewed_endpoint_plus_store_row() {
        let fixture: Value = serde_json::from_slice(include_bytes!(
            "../../../fixtures/das-destinations/v1/cases.json"
        ))
        .expect("fixture parses");
        let bytes = serde_json::to_vec(&fixture["cases"][0]["document"]).expect("document JSON");
        let (reviewed, row) = reviewed_row(&bytes).expect("reviewed destination");
        assert_eq!(reviewed.endpoint_id, row.endpoint_id);
        assert_eq!(reviewed.object_store_id, row.object_store_id);
        assert_eq!(row.status_word, "Ready");
        let rows = visible_destination_rows(&bytes).expect("all visible stores");
        assert_eq!(rows.len(), 3);
        assert!(rows.iter().any(|row| row.status_word == "Read-only"));
    }

    #[test]
    fn revalidation_never_switches_destinations() {
        revalidate_before_commit(&snapshot(), &state(), 500).expect("unchanged reviewed pair");
        let cases: Value = serde_json::from_slice(include_bytes!(
            "../../../fixtures/das-destinations/v1/revalidation-cases.json"
        ))
        .expect("fixture parses");
        for case in cases["cases"].as_array().expect("cases") {
            let current: DestinationAuthorityState =
                serde_json::from_value(case["state"].clone()).expect("state parses");
            let error = revalidate_before_commit(&snapshot(), &current, 500)
                .expect_err("unsafe state must fail");
            assert_eq!(
                error.to_string(),
                case["expected"].as_str().expect("expected"),
                "{}",
                case["id"]
            );
        }
    }
}
