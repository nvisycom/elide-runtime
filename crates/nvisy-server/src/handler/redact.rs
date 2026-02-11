use axum::{
    Router,
    extract::{Multipart, Query, State},
    routing::post,
    Json,
    http::{StatusCode, HeaderMap, header},
    response::IntoResponse,
};
use bytes::Bytes;
use std::sync::Arc;
use nvisy_ontology::redaction::RedactionContext;
use nvisy_engine::runs::RunManager;
use nvisy_detect::actions::detect_dictionary::DictionaryDef;
use crate::service::AppState;
use crate::service::pipeline;

/// Query parameters for the redact endpoint.
#[derive(Debug, serde::Deserialize)]
pub(crate) struct RedactQuery {
    /// Response format: `"json"` (default) or `"binary"`.
    #[serde(default)]
    pub format: Option<String>,
}

/// JSON response for the redact endpoint.
#[derive(Debug, serde::Serialize, schemars::JsonSchema, utoipa::ToSchema)]
pub(crate) struct RedactResponse {
    /// Unique run identifier.
    pub run_id: String,
    /// Base64-encoded redacted file content.
    pub file: String,
    /// Output file name.
    pub file_name: String,
    /// Content type of the output.
    pub content_type: String,
    /// Pipeline execution summary.
    pub summary: pipeline::PipelineSummary,
    /// Audit trail entries.
    pub audit_trail: Vec<serde_json::Value>,
}

/// Submit a file for redaction via multipart upload.
///
/// Parts:
/// - `file` (binary, required): The file to redact
/// - `context` (JSON, optional): RedactionContext with categories, rules, etc.
/// - `dictionaries` (JSON, optional): Array of DictionaryDef for dictionary matching
#[utoipa::path(
    post,
    path = "/api/v1/redact",
    request_body(content_type = "multipart/form-data"),
    params(
        ("format" = Option<String>, Query, description = "Response format: json (default) or binary")
    ),
    responses(
        (status = 200, description = "Redaction completed", body = RedactResponse),
        (status = 400, description = "Bad request"),
        (status = 500, description = "Internal server error")
    )
)]
async fn redact(
    State(run_manager): State<Arc<RunManager>>,
    Query(query): Query<RedactQuery>,
    mut multipart: Multipart,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let (run_id, _cancel_token) = run_manager.create_run().await;
    run_manager.set_running(run_id).await;

    let mut file_bytes: Option<Bytes> = None;
    let mut file_name = String::from("upload");
    let mut content_type = String::from("application/octet-stream");
    let mut context = RedactionContext::default();
    let mut dictionaries: Vec<DictionaryDef> = Vec::new();

    // Parse multipart parts
    while let Ok(Some(field)) = multipart.next_field().await {
        let name = field.name().unwrap_or("").to_string();

        match name.as_str() {
            "file" => {
                if let Some(fname) = field.file_name() {
                    file_name = fname.to_string();
                }
                if let Some(ct) = field.content_type() {
                    content_type = ct.to_string();
                }
                let data = field.bytes().await.map_err(|e| {
                    (
                        StatusCode::BAD_REQUEST,
                        Json(serde_json::json!({ "error": format!("Failed to read file: {e}") })),
                    )
                })?;
                file_bytes = Some(data);
            }
            "context" => {
                let data = field.bytes().await.map_err(|e| {
                    (
                        StatusCode::BAD_REQUEST,
                        Json(serde_json::json!({ "error": format!("Failed to read context: {e}") })),
                    )
                })?;
                context = serde_json::from_slice(&data).map_err(|e| {
                    (
                        StatusCode::BAD_REQUEST,
                        Json(serde_json::json!({ "error": format!("Invalid context JSON: {e}") })),
                    )
                })?;
            }
            "dictionaries" => {
                let data = field.bytes().await.map_err(|e| {
                    (
                        StatusCode::BAD_REQUEST,
                        Json(serde_json::json!({ "error": format!("Failed to read dictionaries: {e}") })),
                    )
                })?;
                dictionaries = serde_json::from_slice(&data).map_err(|e| {
                    (
                        StatusCode::BAD_REQUEST,
                        Json(serde_json::json!({ "error": format!("Invalid dictionaries JSON: {e}") })),
                    )
                })?;
            }
            _ => {
                // Skip unknown fields
            }
        }
    }

    let file_bytes = file_bytes.ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "Missing 'file' part in multipart upload" })),
        )
    })?;

    // Detect content type from file extension if not provided
    if content_type == "application/octet-stream" {
        if let Some(ext) = file_name.rsplit('.').next() {
            content_type = match ext.to_lowercase().as_str() {
                "pdf" => "application/pdf",
                "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
                "html" | "htm" => "text/html",
                "csv" => "text/csv",
                "json" => "application/json",
                "txt" => "text/plain",
                "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
                "xls" => "application/vnd.ms-excel",
                "parquet" => "application/x-parquet",
                "jpg" | "jpeg" => "image/jpeg",
                "png" => "image/png",
                "tiff" => "image/tiff",
                "bmp" => "image/bmp",
                "webp" => "image/webp",
                "mp3" => "audio/mpeg",
                "wav" => "audio/wav",
                _ => "application/octet-stream",
            }
            .to_string();
        }
    }

    // Execute the pipeline
    let result = pipeline::execute_pipeline(
        file_bytes,
        &file_name,
        &content_type,
        &context,
        &dictionaries,
    )
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": format!("Pipeline failed: {e}") })),
        )
    })?;

    // Return binary or JSON based on format query param
    if query.format.as_deref() == Some("binary") {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::CONTENT_TYPE,
            result.content_type.parse().unwrap_or(header::HeaderValue::from_static("application/octet-stream")),
        );
        headers.insert(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{}\"", result.file_name)
                .parse()
                .unwrap_or(header::HeaderValue::from_static("attachment")),
        );
        headers.insert(
            "x-nvisy-run-id",
            run_id.to_string().parse().unwrap(),
        );
        headers.insert(
            "x-nvisy-total-entities",
            result.summary.total_entities.to_string().parse().unwrap(),
        );
        headers.insert(
            "x-nvisy-total-redactions",
            result.summary.total_redactions.to_string().parse().unwrap(),
        );

        Ok((StatusCode::OK, headers, result.content).into_response())
    } else {
        use base64::Engine;
        let encoded = base64::engine::general_purpose::STANDARD.encode(&result.content);

        let response = RedactResponse {
            run_id: run_id.to_string(),
            file: encoded,
            file_name: result.file_name,
            content_type: result.content_type,
            summary: result.summary,
            audit_trail: result.audit_trail,
        };

        Ok((StatusCode::OK, Json(response)).into_response())
    }
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/redact", post(redact))
}
