use axum::{
    Router,
    extract::State,
    routing::post,
    Json,
};
use std::sync::Arc;
use nvisy_core::redaction::RedactionContext;
use nvisy_engine::runs::RunManager;
use crate::service::AppState;

#[derive(serde::Deserialize, schemars::JsonSchema, utoipa::ToSchema)]
pub(crate) struct RedactRequest {
    source: serde_json::Value,
    #[serde(default)]
    #[schema(value_type = Option<Object>)]
    context: Option<RedactionContext>,
    #[serde(default)]
    output: Option<serde_json::Value>,
    #[serde(rename = "policyId")]
    #[serde(default)]
    policy_id: Option<String>,
}

/// Submit a redaction request.
#[utoipa::path(
    post,
    path = "/api/v1/redact",
    request_body = RedactRequest,
    responses(
        (status = 202, description = "Redaction accepted")
    )
)]
async fn redact(
    State(run_manager): State<Arc<RunManager>>,
    Json(_body): Json<RedactRequest>,
) -> (axum::http::StatusCode, Json<serde_json::Value>) {
    let (run_id, _cancel_token) = run_manager.create_run().await;
    run_manager.set_running(run_id).await;

    // TODO: build redaction graph from body and execute

    (
        axum::http::StatusCode::ACCEPTED,
        Json(serde_json::json!({
            "runId": run_id.to_string(),
            "status": "accepted"
        })),
    )
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/redact", post(redact))
}
