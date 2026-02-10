use axum::{
    Router,
    extract::{Path, State},
    routing::{delete, get, post, put},
    Json,
};
use uuid::Uuid;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/policies", post(create_policy))
        .route("/api/v1/policies", get(list_policies))
        .route("/api/v1/policies/{id}", get(get_policy))
        .route("/api/v1/policies/{id}", put(update_policy))
        .route("/api/v1/policies/{id}", delete(delete_policy))
}

#[derive(serde::Deserialize)]
struct CreatePolicyRequest {
    name: String,
    #[serde(default)]
    rules: Vec<serde_json::Value>,
    #[serde(rename = "defaultMethod", default = "default_method")]
    default_method: String,
    #[serde(rename = "defaultConfidenceThreshold", default = "default_threshold")]
    default_confidence_threshold: f64,
}

fn default_method() -> String { "mask".to_string() }
fn default_threshold() -> f64 { 0.5 }

async fn create_policy(
    State(state): State<AppState>,
    Json(body): Json<CreatePolicyRequest>,
) -> (axum::http::StatusCode, Json<serde_json::Value>) {
    let policy = state.policy_store.create(
        body.name,
        body.rules,
        body.default_method,
        body.default_confidence_threshold,
    );
    (
        axum::http::StatusCode::CREATED,
        Json(serde_json::to_value(&policy).unwrap_or_default()),
    )
}

async fn list_policies(
    State(state): State<AppState>,
) -> Json<serde_json::Value> {
    let policies = state.policy_store.list();
    Json(serde_json::to_value(&policies).unwrap_or_default())
}

async fn get_policy(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, axum::http::StatusCode> {
    match state.policy_store.get(id) {
        Some(policy) => Ok(Json(serde_json::to_value(&policy).unwrap_or_default())),
        None => Err(axum::http::StatusCode::NOT_FOUND),
    }
}

#[derive(serde::Deserialize)]
struct UpdatePolicyRequest {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    rules: Option<Vec<serde_json::Value>>,
    #[serde(rename = "defaultMethod")]
    #[serde(default)]
    default_method: Option<String>,
    #[serde(rename = "defaultConfidenceThreshold")]
    #[serde(default)]
    default_confidence_threshold: Option<f64>,
}

async fn update_policy(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdatePolicyRequest>,
) -> Result<Json<serde_json::Value>, axum::http::StatusCode> {
    match state.policy_store.update(id, body.name, body.rules, body.default_method, body.default_confidence_threshold) {
        Some(policy) => Ok(Json(serde_json::to_value(&policy).unwrap_or_default())),
        None => Err(axum::http::StatusCode::NOT_FOUND),
    }
}

async fn delete_policy(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, axum::http::StatusCode> {
    if state.policy_store.delete(id) {
        Ok(Json(serde_json::json!({ "deleted": true })))
    } else {
        Err(axum::http::StatusCode::NOT_FOUND)
    }
}
