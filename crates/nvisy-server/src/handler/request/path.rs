//! Typed path parameters for API endpoints.

use schemars::JsonSchema;
use serde::Deserialize;
use uuid::Uuid;

/// Path parameter carrying a single resource identifier.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ResourcePath {
    /// Resource identifier.
    pub id: Uuid,
}

/// Path parameter for file endpoints.
pub type ContentPath = ResourcePath;

/// Path parameter for context endpoints.
pub type ContextPath = ResourcePath;

/// Path parameter for policy endpoints.
pub type PolicyPath = ResourcePath;

/// Path parameter for run endpoints.
pub type RunPath = ResourcePath;
