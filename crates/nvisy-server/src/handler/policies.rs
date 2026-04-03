//! Policy upload, download, listing, and deletion handlers.
//!
//! # Endpoints
//!
//! | Method   | Path              | Description                    |
//! |----------|-------------------|--------------------------------|
//! | `POST`   | `/policies`       | Upload a policy                |
//! | `GET`    | `/policies`       | List all policies              |
//! | `GET`    | `/policies/{id}`  | Download a previously uploaded policy |
//! | `DELETE` | `/policies/{id}`  | Delete a single policy         |
//! | `DELETE` | `/policies`       | Delete all policies            |
//!
//! Paths are relative — the version prefix (e.g. `/api/v1`) is applied
//! by the version module.

use aide::axum::ApiRouter;
use aide::axum::routing::{get_with, post_with};
use aide::transform::TransformOperation;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use nvisy_engine::registry::Registry;
use nvisy_ontology::policy::Policy;

use super::error::Result;
use super::request::{NewPolicy, Pagination, PolicyPath};
use super::response::{PolicyEntry, PolicyId, PolicyList};
use crate::extract::{ActorId, Json, Path};
use crate::middleware::{DEFAULT_READ_TIMEOUT_SECS, DEFAULT_WRITE_TIMEOUT_SECS, RouterTimeoutExt};
use crate::service::ServiceState;

const TARGET: &str = "nvisy_server::policies";

/// `POST /policies`
#[tracing::instrument(
    target = "nvisy_server::policies",
    skip_all,
    fields(%actor_id),
)]
async fn upload_policy(
    State(registry): State<Registry>,
    ActorId(actor_id): ActorId,
    Json(req): Json<NewPolicy>,
) -> Result<(StatusCode, Json<PolicyId>)> {
    let id = registry.register_policy(actor_id, req.policy).await?;
    tracing::info!(target: TARGET, %id, "policy uploaded");
    Ok((StatusCode::CREATED, Json(PolicyId { id })))
}

fn upload_policy_docs(op: TransformOperation) -> TransformOperation {
    op.id("uploadPolicy")
        .tag("policies")
        .summary("Upload a policy")
        .description("Stores a redaction policy for use in pipeline runs.")
}

/// `GET /policies`
#[tracing::instrument(
    target = "nvisy_server::policies",
    skip_all,
    fields(%actor_id),
)]
async fn list_policies(
    State(registry): State<Registry>,
    ActorId(actor_id): ActorId,
    Query(pagination): Query<Pagination>,
) -> Result<Json<PolicyList>> {
    let ids = registry.list_policies(actor_id).await?;
    let mut entries = Vec::with_capacity(ids.len());
    for id in ids {
        if let Ok(policy) = registry.read_policy(actor_id, id).await {
            entries.push(PolicyEntry {
                id,
                name: policy.name,
                version: policy.version.to_string(),
            });
        }
    }
    let page = pagination.paginate(entries);
    tracing::debug!(target: TARGET, total = page.total, count = page.items.len(), "policies listed");
    Ok(Json(page))
}

fn list_policies_docs(op: TransformOperation) -> TransformOperation {
    op.id("listPolicies")
        .tag("policies")
        .summary("List all uploaded policies")
        .description("Returns a paginated list of policies currently stored.")
}

/// `GET /policies/{id}`
#[tracing::instrument(
    target = "nvisy_server::policies",
    skip_all,
    fields(%id, %actor_id),
)]
async fn download_policy(
    State(registry): State<Registry>,
    ActorId(actor_id): ActorId,
    Path(PolicyPath { id }): Path<PolicyPath>,
) -> Result<Json<Policy>> {
    let policy = registry.read_policy(actor_id, id).await?;
    tracing::debug!(target: TARGET, "policy downloaded");
    Ok(Json(policy))
}

fn download_policy_docs(op: TransformOperation) -> TransformOperation {
    op.id("downloadPolicy")
        .tag("policies")
        .summary("Download a previously uploaded policy")
        .description("Retrieves a policy by its UUID.")
}

/// `DELETE /policies/{id}`
#[tracing::instrument(
    target = "nvisy_server::policies",
    skip_all,
    fields(%id, %actor_id),
)]
async fn delete_policy(
    State(registry): State<Registry>,
    ActorId(actor_id): ActorId,
    Path(PolicyPath { id }): Path<PolicyPath>,
) -> Result<StatusCode> {
    registry.unregister_policy(actor_id, id).await?;
    tracing::info!(target: TARGET, "policy deleted");
    Ok(StatusCode::NO_CONTENT)
}

fn delete_policy_docs(op: TransformOperation) -> TransformOperation {
    op.id("deletePolicy")
        .tag("policies")
        .summary("Delete an uploaded policy")
        .description("Removes a single policy identified by its UUID.")
}

/// `DELETE /policies`
#[tracing::instrument(
    target = "nvisy_server::policies",
    skip_all,
    fields(%actor_id),
)]
async fn delete_all_policies(
    State(registry): State<Registry>,
    ActorId(actor_id): ActorId,
) -> Result<StatusCode> {
    let deleted = registry.unregister_all_policies(actor_id).await?;
    tracing::info!(target: TARGET, deleted, "all policies deleted");
    Ok(StatusCode::NO_CONTENT)
}

fn delete_all_policies_docs(op: TransformOperation) -> TransformOperation {
    op.id("deleteAllPolicies")
        .tag("policies")
        .summary("Delete all uploaded policies")
        .description("Removes every policy currently stored.")
}

/// Policy routes for API v1 (relative paths).
pub fn routes_v1() -> ApiRouter<ServiceState> {
    let read_routes = ApiRouter::new()
        .api_route("/policies", get_with(list_policies, list_policies_docs))
        .api_route(
            "/policies/{id}",
            get_with(download_policy, download_policy_docs),
        )
        .with_timeout(DEFAULT_READ_TIMEOUT_SECS);

    let write_routes = ApiRouter::new()
        .api_route(
            "/policies",
            post_with(upload_policy, upload_policy_docs)
                .delete_with(delete_all_policies, delete_all_policies_docs),
        )
        .api_route(
            "/policies/{id}",
            aide::axum::routing::delete_with(delete_policy, delete_policy_docs),
        )
        .with_timeout(DEFAULT_WRITE_TIMEOUT_SECS);

    read_routes.merge(write_routes)
}
