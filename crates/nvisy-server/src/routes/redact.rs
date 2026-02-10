use axum::{
    Router,
    extract::State,
    routing::post,
    Json,
};
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/redact", post(redact))
}

#[derive(serde::Deserialize)]
struct RedactRequest {
    source: serde_json::Value,
    #[serde(default)]
    detection: Option<serde_json::Value>,
    #[serde(default)]
    output: Option<serde_json::Value>,
    #[serde(rename = "policyId")]
    #[serde(default)]
    policy_id: Option<String>,
}

async fn redact(
    State(state): State<AppState>,
    Json(_body): Json<RedactRequest>,
) -> (axum::http::StatusCode, Json<serde_json::Value>) {
    let (run_id, _cancel_token) = state.run_manager.create_run().await;
    state.run_manager.set_running(run_id).await;

    // TODO: build redaction graph from body and execute

    (
        axum::http::StatusCode::ACCEPTED,
        Json(serde_json::json!({
            "runId": run_id.to_string(),
            "status": "accepted"
        })),
    )
}
