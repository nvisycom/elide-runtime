use std::collections::HashMap;

use aide::axum::IntoApiResponse;
use axum::extract::State;
use axum::Json;
use nvisy_core::{Error, ErrorKind};
use nvisy_engine::compiler::graph::Graph;
use nvisy_engine::connections::Connection;
use nvisy_identify::Policies;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::error::ServerError;
use crate::service::ServiceState;

/// Request body for `POST /api/v1/redaction`.
#[derive(Debug, Deserialize, JsonSchema)]
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
}

/// Response body for `POST /api/v1/redaction`.
#[derive(Debug, Serialize, JsonSchema)]
pub struct RedactionResponse {
    /// Unique run identifier.
    pub run_id: Uuid,
    /// Identifier of the redacted output content.
    pub output_id: Uuid,
    /// Per-source redaction summaries as opaque JSON.
    pub summaries: serde_json::Value,
    /// Audit trail entries as opaque JSON.
    pub audits: serde_json::Value,
}

/// `POST /api/v1/redaction` — run the redaction pipeline on uploaded content.
pub async fn redact(
    State(_state): State<ServiceState>,
    Json(req): Json<RedactionRequest>,
) -> Result<impl IntoApiResponse, ServerError> {
    // Validate policies eagerly so malformed payloads fail fast.
    let _policies: Policies = serde_json::from_value(req.policies)
        .map_err(|e| Error::new(ErrorKind::Validation, format!("invalid policies: {e}")))?;

    // Validate graph and connections are structurally sound.
    let _graph = req.graph;
    let _connections = req.connections;

    Err::<Json<RedactionResponse>, _>(ServerError::from(Error::new(
        ErrorKind::Runtime,
        format!(
            "redaction endpoint not yet implemented (content_id: {}, actor: {})",
            req.content_id,
            req.actor.as_deref().unwrap_or("<none>"),
        ),
    )))
}
