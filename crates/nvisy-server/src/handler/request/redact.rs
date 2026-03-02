//! Redact request types.

use std::collections::HashMap;

use nvisy_engine::Graph;
use nvisy_engine::engine::{Connection, Context};
use schemars::JsonSchema;
use serde::Deserialize;
use uuid::Uuid;

/// Request body for `POST /api/v1/redaction`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RedactionRequest {
    /// Identifier of previously uploaded content.
    pub content_id: Uuid,
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
    /// Reference-data contexts for detection.
    #[serde(default)]
    pub contexts: Vec<Context>,
}
