//! Context upload, download, listing, and deletion handlers.
//!
//! # Endpoints
//!
//! | Method   | Path                       | Description                           |
//! |----------|----------------------------|---------------------------------------|
//! | `POST`   | `/api/v1/contexts`         | Upload a typed context                |
//! | `GET`    | `/api/v1/contexts`         | List all context identifiers          |
//! | `GET`    | `/api/v1/contexts/{id}`    | Download a previously uploaded context|
//! | `DELETE` | `/api/v1/contexts/{id}`    | Delete a single context               |
//! | `DELETE` | `/api/v1/contexts`         | Delete all contexts                   |

use std::time::Duration;

use aide::axum::ApiRouter;
use aide::axum::routing::{get_with, post_with};
use aide::transform::TransformOperation;
use axum::error_handling::HandleErrorLayer;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use nvisy_engine::pipeline::Engine;
use nvisy_ontology::context::Context;
use tower::ServiceBuilder;
use tower::timeout::TimeoutLayer;

use super::error::Result;
use super::request::{ContextPath, NewContext, Pagination};
use super::response::{ContextEntry, ContextId, ContextList};
use crate::extract::{ActorId, Json, Path};
use crate::middleware::constants::{DEFAULT_READ_TIMEOUT_SECS, DEFAULT_WRITE_TIMEOUT_SECS};
use crate::middleware::recovery::handle_error;
use crate::service::ServiceState;

const TARGET: &str = "nvisy_server::contexts";

/// `POST /api/v1/contexts`: upload a typed context.
#[tracing::instrument(
    target = "nvisy_server::contexts",
    skip_all,
    fields(%actor_id),
)]
async fn upload_context(
    State(engine): State<Engine>,
    ActorId(actor_id): ActorId,
    Json(req): Json<NewContext>,
) -> Result<(StatusCode, Json<ContextId>)> {
    let id = engine.upload_context(actor_id, req.context).await?;

    tracing::info!(target: TARGET, %id, "context uploaded");

    Ok((StatusCode::CREATED, Json(ContextId { id })))
}

fn upload_context_docs(op: TransformOperation) -> TransformOperation {
    op.id("uploadContext")
        .tag("contexts")
        .summary("Upload a typed context")
        .description(
            "Accepts a JSON body with a `context` field. The owning actor is \
             identified by the `X-Actor-Id` header.",
        )
}

/// `GET /api/v1/contexts`: list all context identifiers.
#[tracing::instrument(
    target = "nvisy_server::contexts",
    skip_all,
    fields(%actor_id),
)]
async fn list_contexts(
    State(engine): State<Engine>,
    ActorId(actor_id): ActorId,
    Query(pagination): Query<Pagination>,
) -> Result<Json<ContextList>> {
    let ids = engine.list_contexts(actor_id).await?;
    let mut entries = Vec::with_capacity(ids.len());
    for id in ids {
        if let Ok(ctx) = engine.download_context(actor_id, id).await {
            entries.push(ContextEntry {
                id,
                name: ctx.name,
                entries: ctx.entries.len(),
            });
        }
    }
    let page = pagination.paginate(entries);
    tracing::debug!(target: TARGET, total = page.total, count = page.items.len(), "contexts listed");
    Ok(Json(page))
}

fn list_contexts_docs(op: TransformOperation) -> TransformOperation {
    op.id("listContexts")
        .tag("contexts")
        .summary("List all uploaded contexts")
        .description("Returns the identifiers of every context currently stored.")
}

/// `GET /api/v1/contexts/{id}`: download a previously uploaded context.
#[tracing::instrument(
    target = "nvisy_server::contexts",
    skip_all,
    fields(%id, %actor_id),
)]
async fn download_context(
    State(engine): State<Engine>,
    ActorId(actor_id): ActorId,
    Path(ContextPath { id }): Path<ContextPath>,
) -> Result<Json<Context>> {
    let context = engine.download_context(actor_id, id).await?;
    tracing::debug!(target: TARGET, "context downloaded");
    Ok(Json(context))
}

fn download_context_docs(op: TransformOperation) -> TransformOperation {
    op.id("downloadContext")
        .tag("contexts")
        .summary("Download a previously uploaded context")
        .description("Retrieves a context by its UUID, returning the typed Context JSON.")
}

/// `DELETE /api/v1/contexts/{id}`: delete a single context.
#[tracing::instrument(
    target = "nvisy_server::contexts",
    skip_all,
    fields(%id, %actor_id),
)]
async fn delete_context(
    State(engine): State<Engine>,
    ActorId(actor_id): ActorId,
    Path(ContextPath { id }): Path<ContextPath>,
) -> Result<StatusCode> {
    engine.delete_context(actor_id, id).await?;
    tracing::info!(target: TARGET, "context deleted");
    Ok(StatusCode::NO_CONTENT)
}

fn delete_context_docs(op: TransformOperation) -> TransformOperation {
    op.id("deleteContext")
        .tag("contexts")
        .summary("Delete an uploaded context")
        .description("Removes a single context identified by its UUID.")
}

/// `DELETE /api/v1/contexts`: delete all contexts.
#[tracing::instrument(
    target = "nvisy_server::contexts",
    skip_all,
    fields(%actor_id),
)]
async fn delete_all_contexts(
    State(engine): State<Engine>,
    ActorId(actor_id): ActorId,
) -> Result<StatusCode> {
    let deleted = engine.delete_all_contexts(actor_id).await?;
    tracing::info!(target: TARGET, deleted, "all contexts deleted");
    Ok(StatusCode::NO_CONTENT)
}

fn delete_all_contexts_docs(op: TransformOperation) -> TransformOperation {
    op.id("deleteAllContexts")
        .tag("contexts")
        .summary("Delete all uploaded contexts")
        .description("Removes every context currently stored.")
}

/// Context routes.
pub fn routes() -> ApiRouter<ServiceState> {
    let read_routes = ApiRouter::new()
        .api_route(
            "/api/v1/contexts",
            get_with(list_contexts, list_contexts_docs),
        )
        .api_route(
            "/api/v1/contexts/{id}",
            get_with(download_context, download_context_docs),
        )
        .layer(
            ServiceBuilder::new()
                .layer(HandleErrorLayer::new(handle_error))
                .layer(TimeoutLayer::new(Duration::from_secs(
                    DEFAULT_READ_TIMEOUT_SECS,
                ))),
        );

    let write_routes = ApiRouter::new()
        .api_route(
            "/api/v1/contexts",
            post_with(upload_context, upload_context_docs)
                .delete_with(delete_all_contexts, delete_all_contexts_docs),
        )
        .api_route(
            "/api/v1/contexts/{id}",
            aide::axum::routing::delete_with(delete_context, delete_context_docs),
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
