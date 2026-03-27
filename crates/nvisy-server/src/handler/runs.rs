//! Pipeline run creation, inspection, cancellation, and deletion handlers.
//!
//! # Endpoints
//!
//! | Method   | Path                       | Description                          |
//! |----------|----------------------------|--------------------------------------|
//! | `POST`   | `/api/v1/runs`             | Run the full pipeline                |
//! | `GET`    | `/api/v1/runs`             | List runs with optional filters      |
//! | `GET`    | `/api/v1/runs/{id}`        | Get a full run snapshot              |
//! | `POST`   | `/api/v1/runs/{id}/cancel` | Cancel an in-progress run            |
//! | `DELETE` | `/api/v1/runs/{id}`        | Delete a single finished run         |
//! | `DELETE` | `/api/v1/runs`             | Delete all finished runs             |

use std::time::Duration;

use aide::axum::ApiRouter;
use aide::axum::routing::{get_with, post_with};
use aide::transform::TransformOperation;
use axum::error_handling::HandleErrorLayer;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use nvisy_engine::pipeline::{Engine, EngineInput, EngineOutput, RunFilter, RunSnapshot};
use tower::ServiceBuilder;
use tower::timeout::TimeoutLayer;

use super::error::{ErrorKind, Result};
use super::request::{NewRun, RunPath, RunQuery};
use super::response::RunList;
use crate::extract::{ActorId, Json, Path};
use crate::middleware::constants::{DEFAULT_PIPELINE_TIMEOUT_SECS, DEFAULT_READ_TIMEOUT_SECS};
use crate::middleware::recovery::handle_error;
use crate::service::ServiceState;

const TARGET: &str = "nvisy_server::runs";

/// `POST /api/v1/runs`: run the full pipeline on uploaded content.
///
/// Performs extraction, detection, policy evaluation, and redaction
/// on previously uploaded content identified by the graph's Import nodes.
#[tracing::instrument(
    target = "nvisy_server::runs",
    skip_all,
    fields(%actor_id),
)]
async fn create_run(
    State(engine): State<Engine>,
    ActorId(actor_id): ActorId,
    Json(req): Json<NewRun>,
) -> Result<(StatusCode, Json<EngineOutput>)> {
    let input = EngineInput {
        actor_id,
        policies: req.policies,
        graph: req.graph,
        config: req.config,
        dry_run: req.dry_run,
    };

    let output = engine.run(input).await?;

    tracing::info!(
        target: TARGET,
        run_id = %output.run_id,
        entities = output.detection.entities.len(),
        "pipeline complete",
    );

    Ok((StatusCode::CREATED, Json(output)))
}

fn create_run_docs(op: TransformOperation) -> TransformOperation {
    op.id("createRun")
        .tag("runs")
        .summary("Run the full pipeline on uploaded content")
        .description(
            "Runs the complete pipeline (extraction \u{2192} detection \u{2192} policy \
             evaluation \u{2192} redaction) on previously uploaded content.",
        )
}

/// `GET /api/v1/runs`: list runs with optional status filter, scoped to the caller.
#[tracing::instrument(
    target = "nvisy_server::runs",
    skip_all,
    fields(%actor_id, ?query.status),
)]
async fn list_runs(
    State(engine): State<Engine>,
    ActorId(actor_id): ActorId,
    Query(query): Query<RunQuery>,
) -> Result<Json<RunList>> {
    let filter = RunFilter {
        status: query.status,
    };
    let runs = engine.list_runs(actor_id, filter).await;
    let page = query.pagination.paginate(runs);
    tracing::debug!(target: TARGET, total = page.total, count = page.items.len(), "runs listed");
    Ok(Json(page))
}

fn list_runs_docs(op: TransformOperation) -> TransformOperation {
    op.id("listRuns")
        .tag("runs")
        .summary("List pipeline runs")
        .description(
            "Returns a list of run summaries, optionally filtered by status or actor identity.",
        )
}

/// `GET /api/v1/runs/{id}`: get a full run snapshot.
#[tracing::instrument(
    target = "nvisy_server::runs",
    skip_all,
    fields(%actor_id, %id),
)]
async fn get_run(
    State(engine): State<Engine>,
    ActorId(actor_id): ActorId,
    Path(RunPath { id }): Path<RunPath>,
) -> Result<Json<RunSnapshot>> {
    let snapshot = engine
        .get_run(actor_id, id)
        .await
        .ok_or_else(|| ErrorKind::NotFound.with_resource("run"))?;
    tracing::debug!(target: TARGET, "run retrieved");
    Ok(Json(snapshot))
}

fn get_run_docs(op: TransformOperation) -> TransformOperation {
    op.id("getRun")
        .tag("runs")
        .summary("Get a pipeline run")
        .description("Returns the full snapshot of a single run including per-node status.")
}

/// `POST /api/v1/runs/{id}/cancel`: cancel an in-progress run.
#[tracing::instrument(
    target = "nvisy_server::runs",
    skip_all,
    fields(%actor_id, %id),
)]
async fn cancel_run(
    State(engine): State<Engine>,
    ActorId(actor_id): ActorId,
    Path(RunPath { id }): Path<RunPath>,
) -> Result<StatusCode> {
    engine.cancel_run(actor_id, id).await?;
    tracing::info!(target: TARGET, "run cancelled");
    Ok(StatusCode::NO_CONTENT)
}

fn cancel_run_docs(op: TransformOperation) -> TransformOperation {
    op.id("cancelRun")
        .tag("runs")
        .summary("Cancel a pipeline run")
        .description(
            "Requests cancellation of a pending or running pipeline run. \
             Returns 204 on success, 404 if the run does not exist, \
             or 409 if the run has already finished.",
        )
}

/// `DELETE /api/v1/runs/{id}`: delete a single finished run.
#[tracing::instrument(
    target = "nvisy_server::runs",
    skip_all,
    fields(%actor_id, %id),
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
        .description(
            "Removes a single finished run identified by its UUID. \
             Returns 400 if the run is still active.",
        )
}

/// `DELETE /api/v1/runs`: delete all finished runs.
#[tracing::instrument(target = "nvisy_server::runs", skip_all, fields(%actor_id))]
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
            "Removes every finished run from the store. Active runs \
             (pending or running) are preserved.",
        )
}

/// Run routes.
pub fn routes() -> ApiRouter<ServiceState> {
    let pipeline_routes = ApiRouter::new()
        .api_route("/api/v1/runs", post_with(create_run, create_run_docs))
        .layer(
            ServiceBuilder::new()
                .layer(HandleErrorLayer::new(handle_error))
                .layer(TimeoutLayer::new(Duration::from_secs(
                    DEFAULT_PIPELINE_TIMEOUT_SECS,
                ))),
        );

    let read_routes = ApiRouter::new()
        .api_route(
            "/api/v1/runs",
            get_with(list_runs, list_runs_docs).delete_with(delete_all_runs, delete_all_runs_docs),
        )
        .api_route(
            "/api/v1/runs/{id}",
            get_with(get_run, get_run_docs).delete_with(delete_run, delete_run_docs),
        )
        .api_route(
            "/api/v1/runs/{id}/cancel",
            post_with(cancel_run, cancel_run_docs),
        )
        .layer(
            ServiceBuilder::new()
                .layer(HandleErrorLayer::new(handle_error))
                .layer(TimeoutLayer::new(Duration::from_secs(
                    DEFAULT_READ_TIMEOUT_SECS,
                ))),
        );

    pipeline_routes.merge(read_routes)
}
