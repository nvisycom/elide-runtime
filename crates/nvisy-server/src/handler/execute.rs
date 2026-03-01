//! Pipeline execution handler.
//!
//! # Endpoints
//!
//! | Method | Path               | Description                          |
//! |--------|--------------------|--------------------------------------|
//! | `POST` | `/api/v1/execute`  | Run the full redaction pipeline      |
//!
//! Accepts a JSON body with base64-encoded content, policy definitions, and
//! an execution graph describing the pipeline DAG. Runs the complete
//! detection → evaluation → redaction pipeline and returns the combined result.

use aide::axum::ApiRouter;
use aide::axum::routing::post_with;
use aide::transform::TransformOperation;
use axum::extract::State;
use base64::Engine as _;
use nvisy_core::io::{Content, ContentData};
use nvisy_engine::engine::{Engine as _, EngineInput, Policies};

use super::error::{ErrorKind, Result};
use super::request::ExecuteRequest;
use super::response::ExecuteResponse;
use crate::extract::Json;
use crate::service::ServiceState;

/// `POST /api/v1/execute`: run the full pipeline.
#[tracing::instrument(skip_all, fields(filename = req.filename.as_deref()))]
async fn execute(
    State(state): State<ServiceState>,
    Json(req): Json<ExecuteRequest>,
) -> Result<Json<ExecuteResponse>> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(&req.content)
        .map_err(|e| ErrorKind::BadRequest.with_message(format!("invalid base64: {e}")))?;

    let policies: Policies = serde_json::from_value(req.policies)
        .map_err(|e| ErrorKind::BadRequest.with_message(format!("invalid policies: {e}")))?;

    let mut content_data = ContentData::from(bytes);
    if let Some(ref filename) = req.filename
        && let Some(m) = mime_from_filename(filename)
    {
        content_data.mime = Some(m);
    }
    let content = Content::new(content_data);
    let source = state.content_registry().register(content).await?;

    tracing::debug!(content_source = %source.content_source(), "content registered");

    let input = EngineInput {
        source,
        policies,
        graph: req.graph,
        connections: req.connections,
        actor: req.actor,
    };

    let output = state.engine().run(input).await?;
    let run_id = output.run_id;
    let response = ExecuteResponse::from(output);

    tracing::info!(%run_id, "pipeline execution completed");

    Ok(Json(response))
}

fn execute_docs(op: TransformOperation) -> TransformOperation {
    op.id("executePipeline")
        .tag("pipeline")
        .summary("Run the full redaction pipeline")
        .description(
            "Accepts base64-encoded content with policies and an execution graph, \
             runs detection, evaluation, and redaction, and returns the full result.",
        )
}

/// Derive a MIME type from a filename extension.
fn mime_from_filename(filename: &str) -> Option<String> {
    let ext = filename.rsplit('.').next()?;
    let mime = match ext.to_ascii_lowercase().as_str() {
        // Text
        "txt" => "text/plain",
        "csv" => "text/csv",
        "json" => "application/json",
        "xml" => "application/xml",
        "html" | "htm" => "text/html",
        // Documents
        "pdf" => "application/pdf",
        "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "pptx" => "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        // Images
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "tiff" | "tif" => "image/tiff",
        "bmp" => "image/bmp",
        // Audio
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "ogg" => "audio/ogg",
        "flac" => "audio/flac",
        "aac" => "audio/aac",
        // Archives
        "zip" => "application/zip",
        _ => return None,
    };
    Some(mime.to_string())
}

/// Execute routes.
pub fn routes() -> ApiRouter<ServiceState> {
    ApiRouter::new().api_route("/api/v1/execute", post_with(execute, execute_docs))
}
