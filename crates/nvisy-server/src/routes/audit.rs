use axum::{
    Router,
    extract::{Path, Query, State},
    routing::get,
    Json,
};
use uuid::Uuid;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/audit", get(list_audit))
        .route("/api/v1/audit/{run_id}", get(get_audit_by_run))
}

#[derive(serde::Deserialize)]
struct AuditQuery {
    #[serde(rename = "runId")]
    run_id: Option<String>,
    action: Option<String>,
    #[serde(rename = "sourceId")]
    source_id: Option<String>,
    limit: Option<usize>,
    offset: Option<usize>,
}

async fn list_audit(
    State(state): State<AppState>,
    Query(query): Query<AuditQuery>,
) -> Json<serde_json::Value> {
    let records = state.audit_store.query(
        query.run_id.as_deref(),
        query.action.as_deref(),
        query.source_id.as_deref(),
        query.limit.unwrap_or(100),
        query.offset.unwrap_or(0),
    );
    Json(serde_json::to_value(&records).unwrap_or_default())
}

async fn get_audit_by_run(
    State(state): State<AppState>,
    Path(run_id): Path<Uuid>,
) -> Json<serde_json::Value> {
    let records = state.audit_store.get_by_run_id(run_id);
    Json(serde_json::to_value(&records).unwrap_or_default())
}
