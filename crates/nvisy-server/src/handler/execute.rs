use std::collections::HashMap;

use aide::axum::IntoApiResponse;
use aide::openapi::OpenApi;
use axum::extract::State;
use axum::response::IntoResponse;
use axum::{Extension, Json};
use nvisy_core::io::{Content, ContentData};
use nvisy_core::{Error, ErrorKind};
use nvisy_engine::compiler::graph::Graph;
use nvisy_engine::connections::Connection;
use nvisy_engine::engine::{Engine, EngineInput, EngineOutput};
use nvisy_engine::executor::runner::RunOutput;
use nvisy_identify::{Policies, RedactionSummary};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::error::ServerError;
use crate::service::ServiceState;

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

/// Response body for `POST /api/v1/execute`.
#[derive(Debug, Serialize, JsonSchema)]
pub struct ExecuteResponse {
    /// Unique run identifier.
    pub run_id: Uuid,
    /// Detection output as opaque JSON (DetectionOutput lacks JsonSchema).
    pub detection: serde_json::Value,
    /// Policy evaluation as opaque JSON (PolicyEvaluation lacks JsonSchema).
    pub evaluation: serde_json::Value,
    /// Per-source redaction summaries.
    pub summaries: Vec<RedactionSummary>,
    /// Audit trail entries as opaque JSON (Audit uses flatten).
    pub audits: serde_json::Value,
    /// Per-node DAG execution results.
    pub run_output: RunOutput,
}

impl From<EngineOutput> for ExecuteResponse {
    fn from(out: EngineOutput) -> Self {
        Self {
            run_id: out.run_id,
            detection: serde_json::to_value(&out.detection).unwrap_or_default(),
            evaluation: serde_json::to_value(&out.evaluation).unwrap_or_default(),
            summaries: out.summaries,
            audits: serde_json::to_value(&out.audits).unwrap_or_default(),
            run_output: out.run_output,
        }
    }
}

/// `POST /api/v1/execute` — run the full pipeline.
pub async fn execute(
    State(state): State<ServiceState>,
    Json(req): Json<ExecuteRequest>,
) -> Result<impl IntoApiResponse, ServerError> {
    use base64::Engine as _;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(&req.content)
        .map_err(|e| Error::new(ErrorKind::InvalidInput, format!("invalid base64: {e}")))?;

    let policies: Policies = serde_json::from_value(req.policies)
        .map_err(|e| Error::new(ErrorKind::Validation, format!("invalid policies: {e}")))?;

    let mut content_data = ContentData::from(bytes);
    if let Some(ref filename) = req.filename {
        let mime = mime_from_filename(filename);
        if let Some(m) = mime {
            content_data.mime = Some(m);
        }
    }
    let content = Content::new(content_data);
    let source = state.content_registry.register(content).await?;

    let input = EngineInput {
        source,
        policies,
        graph: req.graph,
        connections: req.connections,
        actor: req.actor,
    };

    let output = state.engine.run(input).await?;
    let response = ExecuteResponse::from(output);
    Ok(Json(response))
}

/// `GET /api/v1/openapi.json` — serve the generated OpenAPI spec.
pub async fn openapi_json(Extension(api): Extension<OpenApi>) -> impl IntoResponse {
    Json(api)
}

/// Derive a MIME type from a filename extension.
fn mime_from_filename(filename: &str) -> Option<String> {
    let ext = filename.rsplit('.').next()?;
    let mime = match ext.to_ascii_lowercase().as_str() {
        "txt" => "text/plain",
        "csv" => "text/csv",
        "json" => "application/json",
        "xml" => "application/xml",
        "html" | "htm" => "text/html",
        "pdf" => "application/pdf",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "tiff" | "tif" => "image/tiff",
        "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "pptx" => "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        "zip" => "application/zip",
        _ => return None,
    };
    Some(mime.to_string())
}
