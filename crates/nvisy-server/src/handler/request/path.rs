//! Typed path parameters for API endpoints.
//!
//! One newtype per resource so the OpenAPI schema documents each
//! distinct parameter shape (aide collapses type aliases).

use schemars::JsonSchema;
use semver::Version;
use serde::Deserialize;
use uuid::Uuid;

/// Path parameter for `/files/{id}` endpoints.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct FilePath {
    /// File identifier.
    pub id: Uuid,
}

/// Path parameter for `/detections/{id}` endpoints. Path id is
/// the underlying run id — `/detections` and `/redactions` are
/// filtered views of the same `runs` keyspace.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct DetectionPath {
    /// Run identifier.
    pub id: Uuid,
}

/// Path parameter for `/redactions/{id}` endpoints.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct RedactionPath {
    /// Run identifier.
    pub id: Uuid,
}

/// Path parameter for `/policies/{id}/{version}` endpoints.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct PolicyVersionPath {
    /// Policy identifier.
    pub id: Uuid,
    /// Semver version.
    #[schemars(with = "String")]
    pub version: Version,
}

/// Path parameter for `/policies/{id}/latest` and
/// `/policies/{id}` listing-by-id endpoints.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct PolicyIdPath {
    /// Policy identifier.
    pub id: Uuid,
}

/// Path parameter for `/contexts/{id}/{version}` endpoints.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ContextVersionPath {
    /// Context identifier.
    pub id: Uuid,
    /// Semver version.
    #[schemars(with = "String")]
    pub version: Version,
}

/// Path parameter for `/contexts/{id}/latest` and
/// `/contexts/{id}` listing-by-id endpoints.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ContextIdPath {
    /// Context identifier.
    pub id: Uuid,
}
