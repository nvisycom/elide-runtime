//! Execute request types.

use std::collections::HashMap;

use nvisy_engine::Graph;
use nvisy_engine::engine::Connection;
use schemars::JsonSchema;
use serde::Deserialize;

/// Request body for `POST /api/v1/execute`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ExecuteRequest {
    /// Base64-encoded content bytes.
    pub content: String,
    /// Optional original filename.
    #[serde(default)]
    pub filename: Option<String>,
    /// Policies as opaque JSON (validated in the handler).
    pub policies: serde_json::Value,
    /// Execution graph defining the pipeline DAG.
    pub graph: Graph,
    /// External service connections keyed by ID.
    #[serde(default)]
    pub connections: HashMap<String, Connection>,
    /// Human or service account identity.
    #[serde(default)]
    pub actor: Option<String>,
}
