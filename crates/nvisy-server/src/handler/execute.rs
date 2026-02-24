use aide::axum::IntoApiResponse;
use aide::openapi::OpenApi;
use axum::extract::State;
use axum::response::IntoResponse;
use axum::{Extension, Json};
use nvisy_core::io::{Content, ContentData};
use nvisy_core::{Error, ErrorKind};
use nvisy_engine::engine::{Engine, EngineInput};
use nvisy_identify::Policies;

use super::request::ExecuteRequest;
use super::response::{ExecuteResponse, ServerError};
use crate::service::ServiceState;

/// `POST /api/v1/execute` — run the full pipeline.
#[tracing::instrument(skip_all, fields(filename = req.filename.as_deref()))]
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
    if let Some(ref filename) = req.filename
        && let Some(m) = mime_from_filename(filename)
    {
        content_data.mime = Some(m);
    }
    let content = Content::new(content_data);
    let source = state.content_registry.register(content).await?;

    tracing::debug!(content_source = %source.content_source(), "content registered");

    let input = EngineInput {
        source,
        policies,
        graph: req.graph,
        connections: req.connections,
        actor: req.actor,
    };

    let output = state.engine.run(input).await?;
    let run_id = output.run_id;
    let response = ExecuteResponse::from(output);

    tracing::info!(%run_id, "pipeline execution completed");

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
