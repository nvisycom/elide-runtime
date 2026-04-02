//! Policy upload, download, listing, and deletion handlers.
//!
//! # Endpoints
//!
//! | Method   | Path              | Description                    |
//! |----------|-------------------|--------------------------------|
//! | `POST`   | `/policies`       | Upload a policy                |
//! | `GET`    | `/policies`       | List all policy identifiers    |
//! | `GET`    | `/policies/{id}`  | Download a previously uploaded policy |
//! | `DELETE` | `/policies/{id}`  | Delete a single policy         |
//!
//! Paths are relative — the version prefix (e.g. `/api/v1`) is applied
//! by the version module.

use std::time::Duration;

use aide::axum::ApiRouter;
use aide::axum::routing::{get_with, post_with};
use aide::transform::TransformOperation;
use axum::error_handling::HandleErrorLayer;
use axum::extract::State;
use axum::http::StatusCode;
use nvisy_engine::registry::Registry;
use nvisy_ontology::policy::Policy;
use tower::ServiceBuilder;
use tower::timeout::TimeoutLayer;

use super::error::Result;
use super::request::PolicyPath;
use crate::extract::{ActorId, Json, Path};
use crate::middleware::constants::{DEFAULT_READ_TIMEOUT_SECS, DEFAULT_WRITE_TIMEOUT_SECS};
use crate::middleware::recovery::handle_error;
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
    Json(policy): Json<Policy>,
) -> Result<(StatusCode, Json<serde_json::Value>)> {
    let id = registry.register_policy(actor_id, policy).await?;
    tracing::info!(target: TARGET, %id, "policy uploaded");
    Ok((StatusCode::CREATED, Json(serde_json::json!({ "id": id }))))
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
) -> Result<Json<Vec<uuid::Uuid>>> {
    let ids = registry.list_policies(actor_id).await?;
    tracing::debug!(target: TARGET, count = ids.len(), "policies listed");
    Ok(Json(ids))
}

fn list_policies_docs(op: TransformOperation) -> TransformOperation {
    op.id("listPolicies")
        .tag("policies")
        .summary("List all uploaded policies")
        .description("Returns the identifiers of every policy currently stored.")
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

/// Policy routes for API v1 (relative paths).
pub fn routes_v1() -> ApiRouter<ServiceState> {
    let read_routes = ApiRouter::new()
        .api_route("/policies", get_with(list_policies, list_policies_docs))
        .api_route(
            "/policies/{id}",
            get_with(download_policy, download_policy_docs),
        )
        .layer(
            ServiceBuilder::new()
                .layer(HandleErrorLayer::new(handle_error))
                .layer(TimeoutLayer::new(Duration::from_secs(
                    DEFAULT_READ_TIMEOUT_SECS,
                ))),
        );

    let write_routes = ApiRouter::new()
        .api_route("/policies", post_with(upload_policy, upload_policy_docs))
        .api_route(
            "/policies/{id}",
            aide::axum::routing::delete_with(delete_policy, delete_policy_docs),
        )
        .layer(
            ServiceBuilder::new()
                .layer(HandleErrorLayer::new(handle_error))
                .layer(TimeoutLayer::new(Duration::from_secs(
                    DEFAULT_WRITE_TIMEOUT_SECS,
                ))),
        );

    read_routes.merge(write_routes)
}
