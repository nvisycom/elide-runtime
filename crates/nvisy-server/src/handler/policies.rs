use axum::{
    Router,
    extract::{Path, State},
    routing::{delete, get, post, put},
    Json,
};
use std::sync::Arc;
use uuid::Uuid;
use crate::service::PolicyStore;
use crate::service::AppState;

#[derive(serde::Deserialize, schemars::JsonSchema, utoipa::ToSchema)]
pub(crate) struct CreatePolicyRequest {
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

#[derive(serde::Deserialize, schemars::JsonSchema, utoipa::ToSchema)]
pub(crate) struct UpdatePolicyRequest {
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

/// Create a new policy.
#[utoipa::path(
    post,
    path = "/api/v1/policies",
    request_body = CreatePolicyRequest,
    responses(
        (status = 201, description = "Policy created")
    )
)]
async fn create_policy(
    State(policy_store): State<Arc<PolicyStore>>,
    Json(body): Json<CreatePolicyRequest>,
) -> (axum::http::StatusCode, Json<serde_json::Value>) {
    let policy = policy_store.create(
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

/// List all policies.
#[utoipa::path(
    get,
    path = "/api/v1/policies",
    responses(
        (status = 200, description = "List of policies")
    )
)]
async fn list_policies(
    State(policy_store): State<Arc<PolicyStore>>,
) -> Json<serde_json::Value> {
    let policies = policy_store.list();
    Json(serde_json::to_value(&policies).unwrap_or_default())
}

/// Get a policy by ID.
#[utoipa::path(
    get,
    path = "/api/v1/policies/{id}",
    params(
        ("id" = Uuid, Path, description = "Policy ID")
    ),
    responses(
        (status = 200, description = "Policy details"),
        (status = 404, description = "Policy not found")
    )
)]
async fn get_policy(
    State(policy_store): State<Arc<PolicyStore>>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, axum::http::StatusCode> {
    match policy_store.get(id) {
        Some(policy) => Ok(Json(serde_json::to_value(&policy).unwrap_or_default())),
        None => Err(axum::http::StatusCode::NOT_FOUND),
    }
}

/// Update an existing policy.
#[utoipa::path(
    put,
    path = "/api/v1/policies/{id}",
    params(
        ("id" = Uuid, Path, description = "Policy ID")
    ),
    request_body = UpdatePolicyRequest,
    responses(
        (status = 200, description = "Policy updated"),
        (status = 404, description = "Policy not found")
    )
)]
async fn update_policy(
    State(policy_store): State<Arc<PolicyStore>>,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdatePolicyRequest>,
) -> Result<Json<serde_json::Value>, axum::http::StatusCode> {
    match policy_store.update(id, body.name, body.rules, body.default_method, body.default_confidence_threshold) {
        Some(policy) => Ok(Json(serde_json::to_value(&policy).unwrap_or_default())),
        None => Err(axum::http::StatusCode::NOT_FOUND),
    }
}

/// Delete a policy.
#[utoipa::path(
    delete,
    path = "/api/v1/policies/{id}",
    params(
        ("id" = Uuid, Path, description = "Policy ID")
    ),
    responses(
        (status = 200, description = "Policy deleted"),
        (status = 404, description = "Policy not found")
    )
)]
async fn delete_policy(
    State(policy_store): State<Arc<PolicyStore>>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, axum::http::StatusCode> {
    if policy_store.delete(id) {
        Ok(Json(serde_json::json!({ "deleted": true })))
    } else {
        Err(axum::http::StatusCode::NOT_FOUND)
    }
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/policies", post(create_policy))
        .route("/api/v1/policies", get(list_policies))
        .route("/api/v1/policies/{id}", get(get_policy))
        .route("/api/v1/policies/{id}", put(update_policy))
        .route("/api/v1/policies/{id}", delete(delete_policy))
}
