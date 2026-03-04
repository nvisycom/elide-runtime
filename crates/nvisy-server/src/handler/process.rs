//! Processing pipeline handlers.
//!
//! # Endpoints
//!
//! | Method | Path                       | Description                                    |
//! |--------|----------------------------|------------------------------------------------|
//! | `POST` | `/api/v1/process/scan`     | Run OCR on previously uploaded content         |
//! | `POST` | `/api/v1/process/analyze`  | Run OCR + LLM analysis on uploaded content     |
//! | `POST` | `/api/v1/process/redact`   | Run the full redaction pipeline                |
//!
//! All endpoints expect a JSON body with a `content_id` referencing previously
//! uploaded content, along with policies, an execution graph, and optional
//! connections and contexts.

use aide::axum::ApiRouter;
use aide::axum::routing::post_with;
use aide::transform::TransformOperation;
use axum::extract::State;
use nvisy_engine::pipeline::Policies;

use super::error::{ErrorKind, Result};
use super::request::ProcessRequest;
use super::response::ProcessResponse;
use crate::extract::Json;
use crate::service::ServiceState;

/// `POST /api/v1/process/scan`: run OCR on uploaded content.
///
/// Extracts text and structural information from the content without
/// further classification or redaction.
#[tracing::instrument(skip_all, fields(content_id = %req.content_id, actor = req.actor.as_deref()))]
async fn scan(
    State(_state): State<ServiceState>,
    Json(req): Json<ProcessRequest>,
) -> Result<Json<ProcessResponse>> {
    let _policies: Policies = serde_json::from_value(req.policies)
        .map_err(|e| ErrorKind::BadRequest.with_message(format!("invalid policies: {e}")))?;

    Err(ErrorKind::NotImplemented.with_message(format!(
        "scan endpoint not yet implemented (content_id: {}, actor: {})",
        req.content_id,
        req.actor.as_deref().unwrap_or("<none>"),
    )))
}

fn scan_docs(op: TransformOperation) -> TransformOperation {
    op.id("scanContent")
        .tag("process")
        .summary("Run OCR on uploaded content")
        .description(
            "Runs OCR on previously uploaded content identified by content_id. \
             Extracts text and structural information without classification or redaction.",
        )
}

/// `POST /api/v1/process/analyze`: run OCR + LLM analysis on uploaded content.
///
/// Extracts text via OCR and classifies entities using an LLM, without
/// applying any redactions.
#[tracing::instrument(skip_all, fields(content_id = %req.content_id, actor = req.actor.as_deref()))]
async fn analyze(
    State(_state): State<ServiceState>,
    Json(req): Json<ProcessRequest>,
) -> Result<Json<ProcessResponse>> {
    let _policies: Policies = serde_json::from_value(req.policies)
        .map_err(|e| ErrorKind::BadRequest.with_message(format!("invalid policies: {e}")))?;

    Err(ErrorKind::NotImplemented.with_message(format!(
        "analyze endpoint not yet implemented (content_id: {}, actor: {})",
        req.content_id,
        req.actor.as_deref().unwrap_or("<none>"),
    )))
}

fn analyze_docs(op: TransformOperation) -> TransformOperation {
    op.id("analyzeContent")
        .tag("process")
        .summary("Run OCR + LLM analysis on uploaded content")
        .description(
            "Runs OCR followed by LLM-based entity classification on previously \
             uploaded content. Returns detected entities without applying redactions.",
        )
}

/// `POST /api/v1/process/redact`: run the full redaction pipeline.
///
/// Performs OCR, entity classification, policy evaluation, and redaction
/// on previously uploaded content.
#[tracing::instrument(skip_all, fields(content_id = %req.content_id, actor = req.actor.as_deref()))]
async fn redact(
    State(_state): State<ServiceState>,
    Json(req): Json<ProcessRequest>,
) -> Result<Json<ProcessResponse>> {
    let _policies: Policies = serde_json::from_value(req.policies)
        .map_err(|e| ErrorKind::BadRequest.with_message(format!("invalid policies: {e}")))?;

    Err(ErrorKind::NotImplemented.with_message(format!(
        "redact endpoint not yet implemented (content_id: {}, actor: {})",
        req.content_id,
        req.actor.as_deref().unwrap_or("<none>"),
    )))
}

fn redact_docs(op: TransformOperation) -> TransformOperation {
    op.id("redactContent")
        .tag("process")
        .summary("Run the full redaction pipeline on uploaded content")
        .description(
            "Runs the complete pipeline (OCR → entity classification → policy \
             evaluation → redaction) on previously uploaded content.",
        )
}

/// Process routes.
pub fn routes() -> ApiRouter<ServiceState> {
    ApiRouter::new()
        .api_route("/api/v1/process/scan", post_with(scan, scan_docs))
        .api_route("/api/v1/process/analyze", post_with(analyze, analyze_docs))
        .api_route("/api/v1/process/redact", post_with(redact, redact_docs))
}
