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

use aide::axum::ApiRouter;
use aide::axum::routing::{get_with, post_with};
use aide::transform::TransformOperation;
use axum::extract::{Query, State};
use axum::http::StatusCode;

use nvisy_registry::ContextId;

use super::error::Result;
use super::request::{ActorQuery, ContextPath, ContextUpload};
use super::response::{
    ContextDeleteAllResponse, ContextDeleteResponse, ContextDownloadResponse,
    ContextListResponse, ContextUploadResponse,
};
use crate::extract::{Json, Path};
use crate::service::ServiceState;

/// `POST /api/v1/contexts`: upload a typed context.
#[tracing::instrument(skip_all)]
async fn upload(
    State(state): State<ServiceState>,
    Json(req): Json<ContextUpload>,
) -> Result<(StatusCode, Json<ContextUploadResponse>)> {
    let handle = state
        .registry()
        .register_context(req.actor_id, req.context)
        .await?;
    let id = ContextId::from(handle.source().as_uuid());

    tracing::info!(%id, "context uploaded");

    Ok((StatusCode::CREATED, Json(ContextUploadResponse { id })))
}

fn upload_docs(op: TransformOperation) -> TransformOperation {
    op.id("uploadContext")
        .tag("contexts")
        .summary("Upload a typed context")
        .description(
            "Accepts a JSON body with a `context` field containing the Context struct \
             and an `actorId` identifying the owning actor.",
        )
}

/// `GET /api/v1/contexts`: list all context identifiers.
#[tracing::instrument(skip_all)]
async fn list(
    State(state): State<ServiceState>,
    Query(ActorQuery { actor_id }): Query<ActorQuery>,
) -> Result<Json<ContextListResponse>> {
    let contexts = state.registry().list_contexts(actor_id).await?;
    Ok(Json(ContextListResponse { contexts }))
}

fn list_docs(op: TransformOperation) -> TransformOperation {
    op.id("listContexts")
        .tag("contexts")
        .summary("List all uploaded contexts")
        .description("Returns the identifiers of every context currently stored.")
}

/// `GET /api/v1/contexts/{id}`: download a previously uploaded context.
#[tracing::instrument(skip_all, fields(%id))]
async fn download(
    State(state): State<ServiceState>,
    Path(ContextPath { id }): Path<ContextPath>,
    Query(ActorQuery { actor_id }): Query<ActorQuery>,
) -> Result<Json<ContextDownloadResponse>> {
    let handle = state.registry().read_context(actor_id, id).await?;
    let context = handle.context().await?;
    Ok(Json(ContextDownloadResponse { id, context }))
}

fn download_docs(op: TransformOperation) -> TransformOperation {
    op.id("downloadContext")
        .tag("contexts")
        .summary("Download a previously uploaded context")
        .description("Retrieves a context by its UUID, returning the typed Context JSON.")
}

/// `DELETE /api/v1/contexts/{id}`: delete a single context.
#[tracing::instrument(skip_all, fields(%id))]
async fn delete(
    State(state): State<ServiceState>,
    Path(ContextPath { id }): Path<ContextPath>,
    Query(ActorQuery { actor_id }): Query<ActorQuery>,
) -> Result<Json<ContextDeleteResponse>> {
    state.registry().unregister_context(actor_id, id).await?;
    tracing::info!(%id, "context deleted");
    Ok(Json(ContextDeleteResponse { id }))
}

fn delete_docs(op: TransformOperation) -> TransformOperation {
    op.id("deleteContext")
        .tag("contexts")
        .summary("Delete an uploaded context")
        .description("Removes a single context identified by its UUID.")
}

/// `DELETE /api/v1/contexts`: delete all contexts.
#[tracing::instrument(skip_all)]
async fn delete_all(
    State(state): State<ServiceState>,
    Query(ActorQuery { actor_id }): Query<ActorQuery>,
) -> Result<Json<ContextDeleteAllResponse>> {
    let deleted = state
        .registry()
        .unregister_all_contexts(actor_id)
        .await?;
    tracing::info!(deleted, "all contexts deleted");
    Ok(Json(ContextDeleteAllResponse { deleted }))
}

fn delete_all_docs(op: TransformOperation) -> TransformOperation {
    op.id("deleteAllContexts")
        .tag("contexts")
        .summary("Delete all uploaded contexts")
        .description("Removes every context currently stored.")
}

/// Context routes.
pub fn routes() -> ApiRouter<ServiceState> {
    ApiRouter::new()
        .api_route(
            "/api/v1/contexts",
            post_with(upload, upload_docs)
                .get_with(list, list_docs)
                .delete_with(delete_all, delete_all_docs),
        )
        .api_route(
            "/api/v1/contexts/{id}",
            get_with(download, download_docs).delete_with(delete, delete_docs),
        )
}
