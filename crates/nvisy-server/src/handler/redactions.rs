//! Redaction-side handlers — the "apply the redaction" half of
//! the run lifecycle.
//!
//! `POST /redactions` carries reviewer overrides and triggers
//! the apply transition on the referenced detection. The
//! response carries the per-doc apply outcomes (one redacted
//! file id per successful input). `GET /redactions/{id}`
//! returns the same full run state as `GET /detections/{id}` —
//! they're views on the same underlying run.

use aide::axum::ApiRouter;
use aide::axum::routing::{get_with, post_with};
use aide::transform::TransformOperation;
use axum::extract::State;
use axum::http::StatusCode;
use nvisy_engine::{Engine, runs};

use super::detections::fetch_docs;
use super::error::Result;
use super::request::{NewRedaction, RedactionPath};
use super::response::{RedactionOutput, RedactionResult, RunResponse};
use crate::extract::{ActorId, Json, Path};
use crate::middleware::{DEFAULT_READ_TIMEOUT, DEFAULT_WRITE_TIMEOUT, RouterTimeoutExt};
use crate::service::ServiceState;

const TARGET: &str = "nvisy_server::redactions";

#[tracing::instrument(target = TARGET, skip_all, fields(%actor_id))]
async fn create_redaction(
    State(engine): State<Engine>,
    ActorId(actor_id): ActorId,
    Json(req): Json<NewRedaction>,
) -> Result<(StatusCode, Json<RedactionResult>)> {
    let detection_id = req.detection_id;

    // Apply every reviewer override first. Each is its own
    // engine call so a missing entity surfaces a clean error
    // before we kick off the more-expensive apply fan-out.
    for o in &req.overrides {
        runs::override_entity(
            &engine,
            actor_id,
            detection_id,
            o.doc_id,
            o.entity_id,
            o.action.clone(),
        )
        .await?;
    }

    runs::apply(&engine, actor_id, detection_id).await?;

    // Re-read the run + per-doc rows to render the result. The
    // header + rows now carry the Applied / PartiallyApplied /
    // Failed state per doc plus the output file ids.
    let run = runs::get(&engine, actor_id, detection_id).await?;
    let docs = fetch_docs(&engine, actor_id, &run).await;
    let outputs: Vec<RedactionOutput> = docs
        .into_iter()
        .map(|d| {
            let dto: super::response::runs::RunDocumentDto = d.into();
            RedactionOutput {
                doc_id: dto.id,
                input_file_id: dto.input_file_id,
                output_file_id: dto.output_file_id,
                state: dto.state,
                failure_reason: dto.failure_reason,
            }
        })
        .collect();

    tracing::info!(
        target: TARGET,
        run_id = %detection_id,
        outputs = outputs.len(),
        "redaction applied"
    );
    Ok((
        StatusCode::ACCEPTED,
        Json(RedactionResult {
            id: detection_id,
            outputs,
        }),
    ))
}

fn create_redaction_docs(op: TransformOperation) -> TransformOperation {
    op.id("createRedaction")
        .tag("redactions")
        .summary("Apply a detection")
        .description(
            "Applies reviewer overrides + transitions the referenced detection \
             into the redaction phase. Returns the produced file ids; download \
             via `GET /files/{id}/content`. Re-applying the same detection \
             returns `Conflict`.",
        )
}

#[tracing::instrument(target = TARGET, skip_all, fields(%id, %actor_id))]
async fn get_redaction(
    State(engine): State<Engine>,
    ActorId(actor_id): ActorId,
    Path(RedactionPath { id }): Path<RedactionPath>,
) -> Result<Json<RunResponse>> {
    let run = runs::get(&engine, actor_id, id).await?;
    let docs = fetch_docs(&engine, actor_id, &run).await;
    Ok(Json(RunResponse::assemble(run, docs)))
}

fn get_redaction_docs(op: TransformOperation) -> TransformOperation {
    op.id("getRedaction")
        .tag("redactions")
        .summary("Get the run state for a redaction")
        .description("Same shape as `GET /detections/{id}` — they're views on the same run.")
}

pub fn routes_v1() -> ApiRouter<ServiceState> {
    let read = ApiRouter::new()
        .api_route(
            "/redactions/{id}",
            get_with(get_redaction, get_redaction_docs),
        )
        .with_timeout(DEFAULT_READ_TIMEOUT);

    let write = ApiRouter::new()
        .api_route(
            "/redactions",
            post_with(create_redaction, create_redaction_docs),
        )
        .with_timeout(DEFAULT_WRITE_TIMEOUT);

    read.merge(write)
}
