// SPDX-License-Identifier: MPL-2.0
//! Strict synthetic connector-fixture contract validation.
//!
//! The fixture matrix represents deterministic adapter inputs and expected
//! outcomes only. It is not an X or Instagram client and performs no network,
//! authorization, media, or storage operation.

/// Versioned synthetic fixture schema accepted by this contract.
pub const CONNECTOR_FIXTURE_SCHEMA: &str = "x-img.connector-fixtures.v1";

/// Returns the checked-in synthetic fixture matrix.
#[must_use]
pub const fn fixture_bytes() -> &'static [u8] {
    include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/connectors/v1/scenarios.json"
    ))
}

/// Checks that the synthetic fixture matrix remains complete and secret-free by shape.
pub fn validate_fixture_matrix() -> Result<(), FixtureError> {
    let document: serde_json::Value =
        serde_json::from_slice(fixture_bytes()).map_err(|_| FixtureError::InvalidJson)?;
    if document
        .get("schema_version")
        .and_then(serde_json::Value::as_str)
        != Some(CONNECTOR_FIXTURE_SCHEMA)
    {
        return Err(FixtureError::UnsupportedSchema);
    }
    let scenarios = document
        .get("scenarios")
        .and_then(serde_json::Value::as_array)
        .ok_or(FixtureError::MissingScenarios)?;
    let required = [
        "pagination",
        "edit",
        "deleted",
        "duplicate-media",
        "multiple-variants",
        "rate-limited",
        "authorization-expired",
        "malformed-response",
        "cursor-reset",
    ];
    for source in ["x", "instagram"] {
        for suffix in required {
            let expected = format!("{source}-{suffix}");
            if !scenarios.iter().any(|scenario| {
                scenario.get("id").and_then(serde_json::Value::as_str) == Some(expected.as_str())
            }) {
                return Err(FixtureError::MissingScenarios);
            }
        }
    }
    if serde_json::to_string(&document)
        .map_err(|_| FixtureError::InvalidJson)?
        .to_ascii_lowercase()
        .contains("authorization: bearer")
    {
        return Err(FixtureError::UnsafeFixture);
    }
    Ok(())
}

/// Fixture contract error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixtureError {
    /// Fixture bytes could not be parsed as JSON.
    InvalidJson,
    /// Fixture declares an incompatible versioned schema.
    UnsupportedSchema,
    /// Fixture omits a mandatory source/scenario case.
    MissingScenarios,
    /// Fixture contains a disallowed authorization-shaped literal.
    UnsafeFixture,
}
impl std::fmt::Display for FixtureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "connector fixture validation failed: {self:?}")
    }
}
impl std::error::Error for FixtureError {}

#[cfg(test)]
mod tests {
    use super::validate_fixture_matrix;
    #[test]
    fn fixture_matrix_is_complete_and_synthetic() {
        validate_fixture_matrix().expect("fixture matrix");
    }
}
