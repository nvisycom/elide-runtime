//! Pipeline run creation, inspection, cancellation, and deletion handlers.
//!
//! # Endpoints
//!
//! | Method   | Path                 | Description                          |
//! |----------|----------------------|--------------------------------------|
//! | `POST`   | `/runs`              | Run the full pipeline                |
//! | `GET`    | `/runs`              | List runs with optional filters      |
//! | `GET`    | `/runs/{id}`         | Get a full run snapshot              |
//! | `POST`   | `/runs/{id}/cancel`  | Cancel an in-progress run            |
//! | `DELETE` | `/runs/{id}`         | Delete a single finished run         |
//! | `DELETE` | `/runs`              | Delete all finished runs             |
//!
//! Paths are relative — the version prefix (e.g. `/api/v1`) is applied
//! by the version module.

use aide::axum::ApiRouter;
use aide::axum::routing::{get_with, post_with};
use aide::transform::TransformOperation;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use nvisy_engine::pipeline::{Engine, EngineInput, RunFilter, RunSnapshot};

use super::error::{ErrorKind, Result};
use super::request::{NewRun, RunPath, RunQuery};
use super::response::{RunId, RunList};
use crate::extract::{ActorId, Json, Path};
use crate::middleware::{
    DEFAULT_PIPELINE_TIMEOUT_SECS, DEFAULT_READ_TIMEOUT_SECS, RouterTimeoutExt,
};
use crate::service::ServiceState;

const TARGET: &str = "nvisy_server::runs";

/// `POST /runs`
#[tracing::instrument(
    target = "nvisy_server::runs",
    skip_all,
    fields(%actor_id),
)]
async fn create_run(
    State(engine): State<Engine>,
    ActorId(actor_id): ActorId,
    Json(req): Json<NewRun>,
) -> Result<(StatusCode, Json<RunId>)> {
    let input = EngineInput {
        actor_id,
        policies: req.policies,
        graph: req.graph,
        config: req.config,
        dry_run: req.dry_run,
    };

    let id = engine.submit(input).await?;
    tracing::info!(target: TARGET, %id, "pipeline run submitted");

    Ok((StatusCode::ACCEPTED, Json(RunId { id })))
}

fn create_run_docs(op: TransformOperation) -> TransformOperation {
    op.id("createRun")
        .tag("runs")
        .summary("Execute a redaction pipeline")
        .description(
            "Submits content for the full pipeline: import → detect → evaluate → redact → export. \
             The caller must have previously uploaded content and specify its IDs \
             in the graph's Import nodes.",
        )
}

/// `GET /runs`
#[tracing::instrument(
    target = "nvisy_server::runs",
    skip_all,
    fields(%actor_id),
)]
async fn list_runs(
    State(engine): State<Engine>,
    ActorId(actor_id): ActorId,
    Query(query): Query<RunQuery>,
) -> Result<Json<RunList>> {
    let filter = RunFilter {
        status: query.status,
    };
    let entries = engine.list_runs(actor_id, filter).await;
    let page = query.pagination.paginate(entries);
    tracing::debug!(target: TARGET, total = page.total, count = page.items.len(), "runs listed");
    Ok(Json(page))
}

fn list_runs_docs(op: TransformOperation) -> TransformOperation {
    op.id("listRuns")
        .tag("runs")
        .summary("List pipeline runs")
        .description(
            "Returns pipeline runs for the caller, optionally filtered by status. \
             Supports pagination via `offset` and `limit` query parameters.",
        )
}

/// `GET /runs/{id}`
#[tracing::instrument(
    target = "nvisy_server::runs",
    skip_all,
    fields(%id, %actor_id),
)]
async fn get_run(
    State(engine): State<Engine>,
    ActorId(actor_id): ActorId,
    Path(RunPath { id }): Path<RunPath>,
) -> Result<Json<RunSnapshot>> {
    let snapshot = engine
        .get_run(actor_id, id)
        .await
        .ok_or(ErrorKind::NotFound)?;
    tracing::debug!(target: TARGET, "run snapshot retrieved");
    Ok(Json(snapshot))
}

fn get_run_docs(op: TransformOperation) -> TransformOperation {
    op.id("getRun")
        .tag("runs")
        .summary("Get a pipeline run")
        .description("Returns the full run snapshot including per-node status.")
}

/// `POST /runs/{id}/cancel`
#[tracing::instrument(
    target = "nvisy_server::runs",
    skip_all,
    fields(%id, %actor_id),
)]
async fn cancel_run(
    State(engine): State<Engine>,
    ActorId(actor_id): ActorId,
    Path(RunPath { id }): Path<RunPath>,
) -> Result<StatusCode> {
    engine.cancel_run(actor_id, id).await?;
    tracing::info!(target: TARGET, "run cancelled");
    Ok(StatusCode::ACCEPTED)
}

fn cancel_run_docs(op: TransformOperation) -> TransformOperation {
    op.id("cancelRun")
        .tag("runs")
        .summary("Cancel an in-progress run")
        .description(
            "Triggers cooperative cancellation. The run will abort at the next \
             yield point and transition to `Cancelled` status.",
        )
}

/// `DELETE /runs/{id}`
#[tracing::instrument(
    target = "nvisy_server::runs",
    skip_all,
    fields(%id, %actor_id),
)]
async fn delete_run(
    State(engine): State<Engine>,
    ActorId(actor_id): ActorId,
    Path(RunPath { id }): Path<RunPath>,
) -> Result<StatusCode> {
    engine.delete_run(actor_id, id).await?;
    tracing::info!(target: TARGET, "run deleted");
    Ok(StatusCode::NO_CONTENT)
}

fn delete_run_docs(op: TransformOperation) -> TransformOperation {
    op.id("deleteRun")
        .tag("runs")
        .summary("Delete a finished run")
        .description("Removes a single completed or failed run from the store.")
}

/// `DELETE /runs`
#[tracing::instrument(
    target = "nvisy_server::runs",
    skip_all,
    fields(%actor_id),
)]
async fn delete_all_runs(
    State(engine): State<Engine>,
    ActorId(actor_id): ActorId,
) -> Result<StatusCode> {
    let deleted = engine.delete_all_runs(actor_id).await;
    tracing::info!(target: TARGET, deleted, "all finished runs deleted");
    Ok(StatusCode::NO_CONTENT)
}

fn delete_all_runs_docs(op: TransformOperation) -> TransformOperation {
    op.id("deleteAllRuns")
        .tag("runs")
        .summary("Delete all finished runs")
        .description(
            "Removes every completed, failed, or cancelled run. Active runs \
             (pending or running) are preserved.",
        )
}

/// Run routes for API v1 (relative paths).
pub fn routes_v1() -> ApiRouter<ServiceState> {
    let pipeline_routes = ApiRouter::new()
        .api_route("/runs", post_with(create_run, create_run_docs))
        .with_timeout(DEFAULT_PIPELINE_TIMEOUT_SECS);

    let read_routes = ApiRouter::new()
        .api_route(
            "/runs",
            get_with(list_runs, list_runs_docs).delete_with(delete_all_runs, delete_all_runs_docs),
        )
        .api_route(
            "/runs/{id}",
            get_with(get_run, get_run_docs).delete_with(delete_run, delete_run_docs),
        )
        .api_route("/runs/{id}/cancel", post_with(cancel_run, cancel_run_docs))
        .with_timeout(DEFAULT_READ_TIMEOUT_SECS);

    pipeline_routes.merge(read_routes)
}
