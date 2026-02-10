use axum::{
    Router,
    extract::{Path, State},
    routing::{delete, get, post},
    Json,
};
use std::sync::Arc;
use uuid::Uuid;
use nvisy_engine::runs::RunManager;
use crate::service::AppState;

/// Submit a graph for execution.
#[utoipa::path(
    post,
    path = "/api/v1/graphs/execute",
    request_body = serde_json::Value,
    responses(
        (status = 202, description = "Graph execution accepted")
    )
)]
async fn execute_graph(
    State(run_manager): State<Arc<RunManager>>,
    Json(_body): Json<serde_json::Value>,
) -> (axum::http::StatusCode, Json<serde_json::Value>) {
    let (run_id, _cancel_token) = run_manager.create_run().await;
    run_manager.set_running(run_id).await;

    // TODO: spawn actual graph execution
    (
        axum::http::StatusCode::ACCEPTED,
        Json(serde_json::json!({
            "runId": run_id.to_string(),
            "status": "accepted"
        })),
    )
}

/// Validate a graph definition without executing.
#[utoipa::path(
    post,
    path = "/api/v1/graphs/validate",
    request_body = serde_json::Value,
    responses(
        (status = 200, description = "Validation result")
    )
)]
async fn validate_graph(
    Json(_body): Json<serde_json::Value>,
) -> Json<serde_json::Value> {
    // TODO: validate graph against registry
    Json(serde_json::json!({ "valid": true, "errors": [] }))
}

/// List all runs.
#[utoipa::path(
    get,
    path = "/api/v1/graphs",
    responses(
        (status = 200, description = "List of runs")
    )
)]
async fn list_runs(
    State(run_manager): State<Arc<RunManager>>,
) -> Json<serde_json::Value> {
    let runs = run_manager.list(None).await;
    Json(serde_json::to_value(&runs).unwrap_or_default())
}

/// Get status of a single run.
#[utoipa::path(
    get,
    path = "/api/v1/graphs/{run_id}",
    params(
        ("run_id" = Uuid, Path, description = "Run ID")
    ),
    responses(
        (status = 200, description = "Run details"),
        (status = 404, description = "Run not found")
    )
)]
async fn get_run(
    State(run_manager): State<Arc<RunManager>>,
    Path(run_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, axum::http::StatusCode> {
    match run_manager.get(run_id).await {
        Some(run) => Ok(Json(serde_json::to_value(&run).unwrap_or_default())),
        None => Err(axum::http::StatusCode::NOT_FOUND),
    }
}

/// Cancel a running execution.
#[utoipa::path(
    delete,
    path = "/api/v1/graphs/{run_id}",
    params(
        ("run_id" = Uuid, Path, description = "Run ID")
    ),
    responses(
        (status = 200, description = "Run cancelled"),
        (status = 404, description = "Run not found")
    )
)]
async fn cancel_run(
    State(run_manager): State<Arc<RunManager>>,
    Path(run_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, axum::http::StatusCode> {
    if run_manager.cancel(run_id).await {
        Ok(Json(serde_json::json!({ "cancelled": true })))
    } else {
        Err(axum::http::StatusCode::NOT_FOUND)
    }
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/graphs/execute", post(execute_graph))
        .route("/api/v1/graphs/validate", post(validate_graph))
        .route("/api/v1/graphs", get(list_runs))
        .route("/api/v1/graphs/{run_id}", get(get_run))
        .route("/api/v1/graphs/{run_id}", delete(cancel_run))
}
