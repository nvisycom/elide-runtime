//! Context resource handlers. Symmetric with
//! [`super::policies`].

use aide::axum::ApiRouter;
use aide::axum::routing::{delete_with, get_with, post_with};
use aide::transform::TransformOperation;
use axum::extract::State;
use axum::http::StatusCode;
use nvisy_engine::{ContextRegistry, Engine};
use nvisy_schema::context::Context;

use super::error::Result;
use super::request::{ContextIdPath, ContextVersionPath, NewContext};
use super::response::ContextSummary;
use crate::extract::{ActorId, Json, Path};
use crate::middleware::{DEFAULT_READ_TIMEOUT, DEFAULT_WRITE_TIMEOUT, RouterTimeoutExt};
use crate::service::ServiceState;

const TARGET: &str = "nvisy_server::contexts";

#[tracing::instrument(target = TARGET, skip_all, fields(%actor_id))]
async fn put_context(
    State(engine): State<Engine>,
    ActorId(actor_id): ActorId,
    Json(NewContext(context)): Json<NewContext>,
) -> Result<(StatusCode, Json<ContextSummary>)> {
    engine.registry().put_context(actor_id, &context).await?;
    Ok((
        StatusCode::CREATED,
        Json(ContextSummary {
            id: context.id,
            version: context.version,
        }),
    ))
}

fn put_context_docs(op: TransformOperation) -> TransformOperation {
    op.id("putContext")
        .tag("contexts")
        .summary("Write a new context version")
}

#[tracing::instrument(target = TARGET, skip_all, fields(%id, %version, %actor_id))]
async fn get_context(
    State(engine): State<Engine>,
    ActorId(actor_id): ActorId,
    Path(ContextVersionPath { id, version }): Path<ContextVersionPath>,
) -> Result<Json<Context>> {
    let context = engine.registry().get_context(actor_id, id, version).await?;
    Ok(Json(context))
}

fn get_context_docs(op: TransformOperation) -> TransformOperation {
    op.id("getContext")
        .tag("contexts")
        .summary("Read a specific context version")
}

#[tracing::instrument(target = TARGET, skip_all, fields(%id, %actor_id))]
async fn get_latest_context(
    State(engine): State<Engine>,
    ActorId(actor_id): ActorId,
    Path(ContextIdPath { id }): Path<ContextIdPath>,
) -> Result<Json<Context>> {
    let context = engine.registry().latest_context(actor_id, id).await?;
    Ok(Json(context))
}

fn get_latest_context_docs(op: TransformOperation) -> TransformOperation {
    op.id("getLatestContext")
        .tag("contexts")
        .summary("Read the highest-version context for the id")
}

#[tracing::instrument(target = TARGET, skip_all, fields(%actor_id))]
async fn list_contexts(
    State(engine): State<Engine>,
    ActorId(actor_id): ActorId,
) -> Result<Json<Vec<ContextSummary>>> {
    let paged = engine.registry().list_contexts(actor_id).await?;
    let summaries: Vec<ContextSummary> = paged
        .items
        .into_iter()
        .map(|(id, version)| ContextSummary { id, version })
        .collect();
    Ok(Json(summaries))
}

fn list_contexts_docs(op: TransformOperation) -> TransformOperation {
    op.id("listContexts")
        .tag("contexts")
        .summary("List every (context id, version) for the actor")
}

#[tracing::instrument(target = TARGET, skip_all, fields(%id, %version, %actor_id))]
async fn delete_context(
    State(engine): State<Engine>,
    ActorId(actor_id): ActorId,
    Path(ContextVersionPath { id, version }): Path<ContextVersionPath>,
) -> Result<StatusCode> {
    engine
        .registry()
        .delete_context(actor_id, id, version)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

fn delete_context_docs(op: TransformOperation) -> TransformOperation {
    op.id("deleteContext")
        .tag("contexts")
        .summary("Remove one context version")
}

pub fn routes_v1() -> ApiRouter<ServiceState> {
    let read = ApiRouter::new()
        .api_route("/contexts", get_with(list_contexts, list_contexts_docs))
        .api_route(
            "/contexts/{id}/latest",
            get_with(get_latest_context, get_latest_context_docs),
        )
        .api_route(
            "/contexts/{id}/{version}",
            get_with(get_context, get_context_docs),
        )
        .with_timeout(DEFAULT_READ_TIMEOUT);

    let write = ApiRouter::new()
        .api_route("/contexts", post_with(put_context, put_context_docs))
        .api_route(
            "/contexts/{id}/{version}",
            delete_with(delete_context, delete_context_docs),
        )
        .with_timeout(DEFAULT_WRITE_TIMEOUT);

    read.merge(write)
}
