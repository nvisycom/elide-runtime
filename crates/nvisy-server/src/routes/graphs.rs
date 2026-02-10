use axum::{
    Router,
    extract::{Path, State},
    routing::{delete, get, post},
    Json,
};
use uuid::Uuid;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/graphs/execute", post(execute_graph))
        .route("/api/v1/graphs/validate", post(validate_graph))
        .route("/api/v1/graphs", get(list_runs))
        .route("/api/v1/graphs/{run_id}", get(get_run))
        .route("/api/v1/graphs/{run_id}", delete(cancel_run))
}

async fn execute_graph(
    State(state): State<AppState>,
    Json(_body): Json<serde_json::Value>,
) -> (axum::http::StatusCode, Json<serde_json::Value>) {
    let (run_id, _cancel_token) = state.run_manager.create_run().await;
    state.run_manager.set_running(run_id).await;

    // TODO: spawn actual graph execution
    // For now, return the run ID
    (
        axum::http::StatusCode::ACCEPTED,
        Json(serde_json::json!({
            "runId": run_id.to_string(),
            "status": "accepted"
        })),
    )
}

async fn validate_graph(
    State(_state): State<AppState>,
    Json(_body): Json<serde_json::Value>,
) -> Json<serde_json::Value> {
    // TODO: validate graph against registry
    Json(serde_json::json!({ "valid": true, "errors": [] }))
}

async fn list_runs(
    State(state): State<AppState>,
) -> Json<serde_json::Value> {
    let runs = state.run_manager.list(None).await;
    Json(serde_json::to_value(&runs).unwrap_or_default())
}

async fn get_run(
    State(state): State<AppState>,
    Path(run_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, axum::http::StatusCode> {
    match state.run_manager.get(run_id).await {
        Some(run) => Ok(Json(serde_json::to_value(&run).unwrap_or_default())),
        None => Err(axum::http::StatusCode::NOT_FOUND),
    }
}

async fn cancel_run(
    State(state): State<AppState>,
    Path(run_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, axum::http::StatusCode> {
    if state.run_manager.cancel(run_id).await {
        Ok(Json(serde_json::json!({ "cancelled": true })))
    } else {
        Err(axum::http::StatusCode::NOT_FOUND)
    }
}
