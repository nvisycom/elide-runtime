//! External service connection definitions.
//!
//! A [`Connection`] holds the provider type, credentials, and optional context
//! needed to interact with an external service (e.g. S3, a database).
//! [`Connections`] is a type alias mapping connection IDs to their definitions.

use std::collections::HashMap;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A validated connection to an external service such as S3 or a database.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Connection {
    /// Provider type identifier (e.g. `"s3"`, `"postgres"`).
    #[serde(rename = "type")]
    pub provider_type: String,
    /// Opaque credentials payload specific to the provider.
    pub credentials: serde_json::Value,
    /// Optional provider-specific context (e.g. region, endpoint overrides).
    #[serde(default)]
    pub context: serde_json::Value,
}

/// Map of connection IDs to their [`Connection`] definitions.
pub type Connections = HashMap<String, Connection>;
