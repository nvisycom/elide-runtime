//! Typed path parameters for API endpoints.
//!
//! One newtype per resource so the OpenAPI schema documents each
//! distinct parameter shape (aide collapses type aliases). Every
//! one carries a single UUID `id`.

use schemars::JsonSchema;
use serde::Deserialize;
use uuid::Uuid;

/// Path parameter for `/files/{id}` endpoints.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ContentPath {
    /// Content identifier.
    pub id: Uuid,
}

/// Path parameter for `/detections/{id}` endpoints.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct DetectionPath {
    /// Detection identifier.
    pub id: Uuid,
}

/// Path parameter for `/redactions/{id}` endpoints.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct RedactionPath {
    /// Redaction identifier.
    pub id: Uuid,
}
