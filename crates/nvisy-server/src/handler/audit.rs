use axum::{
    Router,
    extract::{Path, Query, State},
    routing::get,
    Json,
};
use std::sync::Arc;
use uuid::Uuid;
use crate::service::AuditStore;
use crate::service::AppState;

#[derive(serde::Deserialize, schemars::JsonSchema, utoipa::IntoParams)]
struct AuditQuery {
    #[serde(rename = "runId")]
    run_id: Option<String>,
    action: Option<String>,
    #[serde(rename = "sourceId")]
    source_id: Option<String>,
    limit: Option<usize>,
    offset: Option<usize>,
}

/// List audit records with optional filters.
#[utoipa::path(
    get,
    path = "/api/v1/audit",
    params(AuditQuery),
    responses(
        (status = 200, description = "List of audit records")
    )
)]
async fn list_audit(
    State(audit_store): State<Arc<AuditStore>>,
    Query(query): Query<AuditQuery>,
) -> Json<serde_json::Value> {
    let records = audit_store.query(
        query.run_id.as_deref(),
        query.action.as_deref(),
        query.source_id.as_deref(),
        query.limit.unwrap_or(100),
        query.offset.unwrap_or(0),
    );
    Json(serde_json::to_value(&records).unwrap_or_default())
}

/// Get audit records for a specific run.
#[utoipa::path(
    get,
    path = "/api/v1/audit/{run_id}",
    params(
        ("run_id" = Uuid, Path, description = "Run ID")
    ),
    responses(
        (status = 200, description = "Audit records for the run")
    )
)]
async fn get_audit_by_run(
    State(audit_store): State<Arc<AuditStore>>,
    Path(run_id): Path<Uuid>,
) -> Json<serde_json::Value> {
    let records = audit_store.get_by_run_id(run_id);
    Json(serde_json::to_value(&records).unwrap_or_default())
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/audit", get(list_audit))
        .route("/api/v1/audit/{run_id}", get(get_audit_by_run))
}
