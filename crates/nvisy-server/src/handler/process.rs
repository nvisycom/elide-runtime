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
//! All endpoints expect a JSON body with `content_ids` referencing previously
//! uploaded content, along with policies and an execution graph.

use aide::axum::ApiRouter;
use aide::axum::routing::post_with;
use aide::transform::TransformOperation;
use axum::extract::State;
use nvisy_engine::pipeline::{DefaultEngine, Engine, EngineInput};

use super::error::Result;
use super::request::NewProcess;
use super::response::ProcessResult;
use crate::extract::Json;
use crate::service::ServiceState;

const TARGET: &str = "nvisy_server::process";

/// Build an [`EngineInput`] from a [`NewProcess`].
fn engine_input(req: NewProcess) -> EngineInput {
    EngineInput {
        actor: req.actor_id,
        content_ids: req.content_ids,
        policies: req.policies,
        graph: req.graph,
        contexts: Vec::new(),
    }
}

/// `POST /api/v1/process/scan`: run OCR on uploaded content.
///
/// Extracts text and structural information from the content without
/// further classification or redaction.
#[tracing::instrument(
    target = "nvisy_server::process",
    skip_all,
    fields(%req.actor_id, content_count = req.content_ids.len(), mode = "scan"),
)]
async fn scan(
    State(engine): State<DefaultEngine>,
    Json(req): Json<NewProcess>,
) -> Result<Json<ProcessResult>> {
    let input = engine_input(req);
    let output = engine.run(input).await?;

    tracing::info!(
        target: TARGET,
        run_id = %output.run_id,
        "scan complete",
    );

    Ok(Json(ProcessResult {
        run_id: output.run_id,
        summaries: serde_json::to_value(&output.summaries).unwrap_or_default(),
        audits: serde_json::to_value(&output.file_audits).unwrap_or_default(),
    }))
}

fn scan_docs(op: TransformOperation) -> TransformOperation {
    op.id("scanContent")
        .tag("process")
        .summary("Run OCR on uploaded content")
        .description(
            "Runs OCR on previously uploaded content identified by content_ids. \
             Extracts text and structural information without classification or redaction.",
        )
}

/// `POST /api/v1/process/analyze`: run OCR + LLM analysis on uploaded content.
///
/// Extracts text via OCR and classifies entities using an LLM, without
/// applying any redactions.
#[tracing::instrument(
    target = "nvisy_server::process",
    skip_all,
    fields(%req.actor_id, content_count = req.content_ids.len(), mode = "analyze"),
)]
async fn analyze(
    State(engine): State<DefaultEngine>,
    Json(req): Json<NewProcess>,
) -> Result<Json<ProcessResult>> {
    let input = engine_input(req);
    let output = engine.run(input).await?;

    tracing::info!(
        target: TARGET,
        run_id = %output.run_id,
        "analysis complete",
    );

    Ok(Json(ProcessResult {
        run_id: output.run_id,
        summaries: serde_json::to_value(&output.summaries).unwrap_or_default(),
        audits: serde_json::to_value(&output.file_audits).unwrap_or_default(),
    }))
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
#[tracing::instrument(
    target = "nvisy_server::process",
    skip_all,
    fields(%req.actor_id, content_count = req.content_ids.len(), mode = "redact"),
)]
async fn redact(
    State(engine): State<DefaultEngine>,
    Json(req): Json<NewProcess>,
) -> Result<Json<ProcessResult>> {
    let input = engine_input(req);
    let output = engine.run(input).await?;

    tracing::info!(
        target: TARGET,
        run_id = %output.run_id,
        "redaction complete",
    );

    Ok(Json(ProcessResult {
        run_id: output.run_id,
        summaries: serde_json::to_value(&output.summaries).unwrap_or_default(),
        audits: serde_json::to_value(&output.file_audits).unwrap_or_default(),
    }))
}

fn redact_docs(op: TransformOperation) -> TransformOperation {
    op.id("redactContent")
        .tag("process")
        .summary("Run the full redaction pipeline on uploaded content")
        .description(
            "Runs the complete pipeline (OCR \u{2192} entity classification \u{2192} policy \
             evaluation \u{2192} redaction) on previously uploaded content.",
        )
}

/// Process routes.
pub fn routes() -> ApiRouter<ServiceState> {
    ApiRouter::new()
        .api_route("/api/v1/process/scan", post_with(scan, scan_docs))
        .api_route("/api/v1/process/analyze", post_with(analyze, analyze_docs))
        .api_route("/api/v1/process/redact", post_with(redact, redact_docs))
}
