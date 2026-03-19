//! Pipeline run creation, inspection, and cancellation handlers.
//!
//! # Endpoints
//!
//! | Method | Path                       | Description                          |
//! |--------|----------------------------|--------------------------------------|
//! | `POST` | `/api/v1/runs`             | Run the full pipeline                |
//! | `POST` | `/api/v1/runs/scan`        | Run a read-only scan (no redaction)  |
//! | `GET`  | `/api/v1/runs`             | List runs with optional filters      |
//! | `GET`  | `/api/v1/runs/{id}`        | Get a full run snapshot              |
//! | `POST` | `/api/v1/runs/{id}/cancel` | Cancel an in-progress run            |

use aide::axum::ApiRouter;
use aide::axum::routing::{get_with, post_with};
use aide::transform::TransformOperation;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use nvisy_engine::pipeline::{DefaultEngine, Engine, EngineInput, EngineOutput, EngineRuns, RunFilter};

use super::error::{ErrorKind, Result};
use super::request::{NewRun, RunPath};
use super::response::{Run, RunList, RunResult};
use crate::extract::{ActorId, Json, Path};
use crate::service::ServiceState;

/// Optional query parameters for listing runs.
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RunQuery {
    /// Filter by run status (e.g. `running`, `succeeded`).
    #[serde(default)]
    pub status: Option<nvisy_engine::pipeline::RunStatus>,
    /// Filter by actor identity.
    #[serde(default)]
    pub actor_id: Option<uuid::Uuid>,
}

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
    State(engine): State<DefaultEngine>,
    ActorId(actor_id): ActorId,
    Json(req): Json<NewRun>,
) -> Result<(StatusCode, Json<RunResult>)> {
    let output = execute_pipeline(&engine, actor_id, req).await?;

    tracing::info!(
        target: TARGET,
        run_id = %output.run_id,
        entities = output.detection.entities.len(),
        "pipeline complete",
    );

    Ok((StatusCode::CREATED, Json(output.into())))
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

/// `POST /api/v1/runs/scan`: run a read-only scan on uploaded content.
///
/// Extracts text and detects entities without applying redactions.
/// The pipeline behaviour is determined by the graph in the request body.
#[tracing::instrument(
    target = "nvisy_server::runs",
    skip_all,
    fields(%actor_id, mode = "scan"),
)]
async fn scan_content(
    State(engine): State<DefaultEngine>,
    ActorId(actor_id): ActorId,
    Json(req): Json<NewRun>,
) -> Result<(StatusCode, Json<RunResult>)> {
    let output = execute_pipeline(&engine, actor_id, req).await?;

    tracing::info!(
        target: TARGET,
        run_id = %output.run_id,
        entities = output.detection.entities.len(),
        "scan complete",
    );

    Ok((StatusCode::CREATED, Json(output.into())))
}

fn scan_content_docs(op: TransformOperation) -> TransformOperation {
    op.id("scanContent")
        .tag("runs")
        .summary("Run a read-only scan on uploaded content")
        .description(
            "Extracts text and detects entities without applying redactions. \
             The pipeline behaviour is determined by the graph in the request body.",
        )
}

/// `GET /api/v1/runs`: list runs with optional status/actor filters.
#[tracing::instrument(
    target = "nvisy_server::runs",
    skip_all,
    fields(?query.status, ?query.actor_id),
)]
async fn list_runs(
    State(engine): State<DefaultEngine>,
    Query(query): Query<RunQuery>,
) -> Result<Json<RunList>> {
    let filter = RunFilter {
        status: query.status,
        actor_id: query.actor_id,
    };
    let runs = engine.list_runs(filter).await;
    tracing::debug!(target: TARGET, count = runs.len(), "runs listed");
    Ok(Json(RunList { runs }))
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
    fields(%id),
)]
async fn get_run(
    State(engine): State<DefaultEngine>,
    Path(RunPath { id }): Path<RunPath>,
) -> Result<Json<Run>> {
    let run = engine
        .get_run(id)
        .await
        .ok_or_else(|| ErrorKind::NotFound.with_resource("run"))?;
    tracing::debug!(target: TARGET, "run retrieved");
    Ok(Json(Run { run }))
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
    fields(%id),
)]
async fn cancel_run(
    State(engine): State<DefaultEngine>,
    Path(RunPath { id }): Path<RunPath>,
) -> Result<StatusCode> {
    engine.cancel_run(id).await?;
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

/// Execute the pipeline for both `create_run` and `scan_content`.
async fn execute_pipeline(
    engine: &DefaultEngine,
    actor_id: uuid::Uuid,
    req: NewRun,
) -> Result<EngineOutput> {
    let input = EngineInput {
        actor_id,
        policies: req.policies,
        graph: req.graph,
        config: req.config,
    };

    Ok(engine.run(input).await?)
}

/// Run routes.
pub fn routes() -> ApiRouter<ServiceState> {
    ApiRouter::new()
        .api_route(
            "/api/v1/runs",
            post_with(create_run, create_run_docs).get_with(list_runs, list_runs_docs),
        )
        .api_route(
            "/api/v1/runs/scan",
            post_with(scan_content, scan_content_docs),
        )
        .api_route("/api/v1/runs/{id}", get_with(get_run, get_run_docs))
        .api_route(
            "/api/v1/runs/{id}/cancel",
            post_with(cancel_run, cancel_run_docs),
        )
}
