//! Policy resource handlers.

use aide::axum::ApiRouter;
use aide::axum::routing::{delete_with, get_with, post_with};
use aide::transform::TransformOperation;
use axum::extract::State;
use axum::http::StatusCode;
use nvisy_core::policy::Policy;
use nvisy_engine::{EngineHandle, PolicyRegistry};

use super::error::Result;
use super::request::{NewPolicy, PolicyIdPath, PolicyVersionPath};
use super::response::PolicySummary;
use crate::extract::{ActorId, Json, Path};
use crate::middleware::{DEFAULT_READ_TIMEOUT, DEFAULT_WRITE_TIMEOUT, RouterTimeoutExt};
use crate::service::ServiceState;

const TARGET: &str = "nvisy_server::policies";

#[tracing::instrument(target = TARGET, skip_all, fields(%actor_id))]
async fn put_policy(
    State(engine): State<EngineHandle>,
    ActorId(actor_id): ActorId,
    Json(NewPolicy(policy)): Json<NewPolicy>,
) -> Result<(StatusCode, Json<PolicySummary>)> {
    engine.registry().put_policy(actor_id, &policy).await?;
    Ok((
        StatusCode::CREATED,
        Json(PolicySummary {
            id: policy.id,
            version: policy.version,
        }),
    ))
}

fn put_policy_docs(op: TransformOperation) -> TransformOperation {
    op.id("putPolicy")
        .tag("policies")
        .summary("Write a new policy version")
        .description(
            "Stores the full policy under `(actor_id, policy.id, policy.version)`. \
             Returns `Conflict` if the same `(id, version)` already exists.",
        )
}

#[tracing::instrument(target = TARGET, skip_all, fields(%id, %version, %actor_id))]
async fn get_policy(
    State(engine): State<EngineHandle>,
    ActorId(actor_id): ActorId,
    Path(PolicyVersionPath { id, version }): Path<PolicyVersionPath>,
) -> Result<Json<Policy>> {
    let policy = engine.registry().get_policy(actor_id, id, version).await?;
    Ok(Json(policy))
}

fn get_policy_docs(op: TransformOperation) -> TransformOperation {
    op.id("getPolicy")
        .tag("policies")
        .summary("Read a specific policy version")
}

#[tracing::instrument(target = TARGET, skip_all, fields(%id, %actor_id))]
async fn get_latest_policy(
    State(engine): State<EngineHandle>,
    ActorId(actor_id): ActorId,
    Path(PolicyIdPath { id }): Path<PolicyIdPath>,
) -> Result<Json<Policy>> {
    let policy = engine.registry().latest_policy(actor_id, id).await?;
    Ok(Json(policy))
}

fn get_latest_policy_docs(op: TransformOperation) -> TransformOperation {
    op.id("getLatestPolicy")
        .tag("policies")
        .summary("Read the highest-version policy for the id")
}

#[tracing::instrument(target = TARGET, skip_all, fields(%actor_id))]
async fn list_policies(
    State(engine): State<EngineHandle>,
    ActorId(actor_id): ActorId,
) -> Result<Json<Vec<PolicySummary>>> {
    let paged = engine.registry().list_policies(actor_id).await?;
    let summaries: Vec<PolicySummary> = paged
        .items
        .into_iter()
        .map(|(id, version)| PolicySummary { id, version })
        .collect();
    Ok(Json(summaries))
}

fn list_policies_docs(op: TransformOperation) -> TransformOperation {
    op.id("listPolicies")
        .tag("policies")
        .summary("List every (policy id, version) for the actor")
}

#[tracing::instrument(target = TARGET, skip_all, fields(%id, %version, %actor_id))]
async fn delete_policy(
    State(engine): State<EngineHandle>,
    ActorId(actor_id): ActorId,
    Path(PolicyVersionPath { id, version }): Path<PolicyVersionPath>,
) -> Result<StatusCode> {
    engine
        .registry()
        .delete_policy(actor_id, id, version)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

fn delete_policy_docs(op: TransformOperation) -> TransformOperation {
    op.id("deletePolicy")
        .tag("policies")
        .summary("Remove one policy version")
}

pub fn routes_v1() -> ApiRouter<ServiceState> {
    let read = ApiRouter::new()
        .api_route("/policies", get_with(list_policies, list_policies_docs))
        .api_route(
            "/policies/{id}/latest",
            get_with(get_latest_policy, get_latest_policy_docs),
        )
        .api_route(
            "/policies/{id}/{version}",
            get_with(get_policy, get_policy_docs),
        )
        .with_timeout(DEFAULT_READ_TIMEOUT);

    let write = ApiRouter::new()
        .api_route("/policies", post_with(put_policy, put_policy_docs))
        .api_route(
            "/policies/{id}/{version}",
            delete_with(delete_policy, delete_policy_docs),
        )
        .with_timeout(DEFAULT_WRITE_TIMEOUT);

    read.merge(write)
}
