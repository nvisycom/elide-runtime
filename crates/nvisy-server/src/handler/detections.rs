//! Detection-side handlers — the "find entities" half of the
//! run lifecycle.
//!
//! `POST /detections` starts a run from already-uploaded file
//! ids. `GET /detections/{id}` returns the full run state
//! (header + every per-document body inline); the response
//! shape covers any [`RunState`] — clients filter on
//! [`state`](super::response::RunStateDto) to render the
//! detection vs redaction view of the same underlying run.

use aide::axum::ApiRouter;
use aide::axum::routing::{delete_with, get_with, post_with};
use aide::transform::TransformOperation;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use futures::future;
use nvisy_engine::{EngineHandle, runs};

use super::error::Result;
use super::request::{DetectionPath, DetectionQuery, NewDetection};
use super::response::{DetectionId, Page, RunResponse};
use crate::extract::{ActorId, Json, Path};
use crate::middleware::{DEFAULT_READ_TIMEOUT, DEFAULT_WRITE_TIMEOUT, RouterTimeoutExt};
use crate::service::ServiceState;

const TARGET: &str = "nvisy_server::detections";

#[tracing::instrument(target = TARGET, skip_all, fields(%actor_id))]
async fn create_detection(
    State(state): State<ServiceState>,
    ActorId(actor_id): ActorId,
    Json(req): Json<NewDetection>,
) -> Result<(StatusCode, Json<DetectionId>)> {
    let batch = req.into_engine_input(state.analyzer_default());
    let id = runs::start(state.engine(), actor_id, batch).await?;
    tracing::info!(target: TARGET, %id, "detection started");
    Ok((StatusCode::ACCEPTED, Json(DetectionId { id })))
}

fn create_detection_docs(op: TransformOperation) -> TransformOperation {
    op.id("createDetection")
        .tag("detections")
        .summary("Start a detection run")
        .description(
            "Runs the analyzer fan-out across the referenced files. Stops at \
             `AwaitingReview`; the matching `POST /redactions` carries reviewer \
             overrides and transitions to `Applied` / `PartiallyApplied`.",
        )
}

#[tracing::instrument(target = TARGET, skip_all, fields(%actor_id))]
async fn list_detections(
    State(engine): State<EngineHandle>,
    ActorId(actor_id): ActorId,
    Query(query): Query<DetectionQuery>,
) -> Result<Json<Page<RunResponse>>> {
    let runs_list = runs::list(&engine, actor_id).await;
    let assembled = assemble_runs(&engine, actor_id, runs_list).await;
    Ok(Json(Page::paginate(assembled, &query.pagination)))
}

fn list_detections_docs(op: TransformOperation) -> TransformOperation {
    op.id("listDetections")
        .tag("detections")
        .summary("List runs")
        .description("Returns every run for the actor, with per-document bodies inlined.")
}

#[tracing::instrument(target = TARGET, skip_all, fields(%id, %actor_id))]
async fn get_detection(
    State(engine): State<EngineHandle>,
    ActorId(actor_id): ActorId,
    Path(DetectionPath { id }): Path<DetectionPath>,
) -> Result<Json<RunResponse>> {
    let run = runs::get(&engine, actor_id, id).await?;
    let docs = fetch_docs(&engine, actor_id, &run).await;
    Ok(Json(RunResponse::assemble(run, docs)))
}

fn get_detection_docs(op: TransformOperation) -> TransformOperation {
    op.id("getDetection")
        .tag("detections")
        .summary("Get the full run state")
        .description("Returns the run header + every per-document body inline.")
}

#[tracing::instrument(target = TARGET, skip_all, fields(%id, %actor_id))]
async fn cancel_detection(
    State(engine): State<EngineHandle>,
    ActorId(actor_id): ActorId,
    Path(DetectionPath { id }): Path<DetectionPath>,
) -> Result<StatusCode> {
    runs::cancel(&engine, actor_id, id).await?;
    Ok(StatusCode::ACCEPTED)
}

fn cancel_detection_docs(op: TransformOperation) -> TransformOperation {
    op.id("cancelDetection")
        .tag("detections")
        .summary("Mark a run as cancelled")
        .description(
            "Sets the run header to `Failed` with `reason = \"cancelled\"`. Only \
             valid from `Analyzing` / `AwaitingReview`. In-flight per-doc fan-out \
             tasks are not interrupted today; cooperative cancellation needs a \
             `CancellationToken` plumbed through the pipeline.",
        )
}

#[tracing::instrument(target = TARGET, skip_all, fields(%id, %actor_id))]
async fn delete_detection(
    State(engine): State<EngineHandle>,
    ActorId(actor_id): ActorId,
    Path(DetectionPath { id }): Path<DetectionPath>,
) -> Result<StatusCode> {
    runs::delete(&engine, actor_id, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

fn delete_detection_docs(op: TransformOperation) -> TransformOperation {
    op.id("deleteDetection")
        .tag("detections")
        .summary("Delete a run")
        .description(
            "Removes the run header + per-doc bodies. Does **not** cascade to \
             input or redacted output files — files are first-class resources \
             and survive their producing run.",
        )
}

pub fn routes_v1() -> ApiRouter<ServiceState> {
    let read = ApiRouter::new()
        .api_route(
            "/detections",
            get_with(list_detections, list_detections_docs),
        )
        .api_route(
            "/detections/{id}",
            get_with(get_detection, get_detection_docs),
        )
        .with_timeout(DEFAULT_READ_TIMEOUT);

    let write = ApiRouter::new()
        .api_route(
            "/detections",
            post_with(create_detection, create_detection_docs),
        )
        .api_route(
            "/detections/{id}",
            delete_with(delete_detection, delete_detection_docs),
        )
        .api_route(
            "/detections/{id}/cancel",
            post_with(cancel_detection, cancel_detection_docs),
        )
        .with_timeout(DEFAULT_WRITE_TIMEOUT);

    read.merge(write)
}

/// Fetch every per-doc row for one run, concurrently. Failures
/// on individual rows are dropped — the run header already
/// records per-doc state, and a missing row means the run was
/// concurrently deleted underneath us.
pub(super) async fn fetch_docs(
    engine: &EngineHandle,
    actor_id: uuid::Uuid,
    run: &nvisy_engine::runs::Run,
) -> Vec<nvisy_engine::runs::RunDocument> {
    let lookups = run
        .document_ids
        .iter()
        .map(|doc_id| runs::get_doc(engine, actor_id, run.id, *doc_id));
    future::join_all(lookups)
        .await
        .into_iter()
        .filter_map(std::result::Result::ok)
        .collect()
}

/// Assemble [`RunResponse`]s for many runs by fetching per-doc
/// rows concurrently per run.
async fn assemble_runs(
    engine: &EngineHandle,
    actor_id: uuid::Uuid,
    runs_list: Vec<nvisy_engine::runs::Run>,
) -> Vec<RunResponse> {
    let mut out = Vec::with_capacity(runs_list.len());
    for run in runs_list {
        let docs = fetch_docs(engine, actor_id, &run).await;
        out.push(RunResponse::assemble(run, docs));
    }
    out
}
