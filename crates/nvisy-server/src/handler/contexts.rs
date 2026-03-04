//! Context upload, download, listing, and deletion handlers.
//!
//! # Endpoints
//!
//! | Method   | Path                       | Description                           |
//! |----------|----------------------------|---------------------------------------|
//! | `POST`   | `/api/v1/contexts`         | Upload a context (base64 JSON)        |
//! | `GET`    | `/api/v1/contexts`         | List all context identifiers          |
//! | `GET`    | `/api/v1/contexts/{id}`    | Download a previously uploaded context|
//! | `DELETE` | `/api/v1/contexts/{id}`    | Delete a single context               |
//! | `DELETE` | `/api/v1/contexts`         | Delete all contexts                   |

use aide::axum::ApiRouter;
use aide::axum::routing::{get_with, post_with};
use aide::transform::TransformOperation;
use axum::extract::State;

use super::error::{ErrorKind, Result};
use super::request::{ContentPath, ContextUpload};
use super::response::{
    ContextDeleteAllResponse, ContextDeleteResponse, ContextDownloadResponse,
    ContextListResponse, ContextUploadResponse,
};
use crate::extract::{Json, Path};
use crate::service::ServiceState;

/// `POST /api/v1/contexts`: upload a context as base64-encoded JSON.
#[tracing::instrument(skip_all, fields(filename = req.filename.as_deref()))]
async fn upload(
    State(_state): State<ServiceState>,
    Json(req): Json<ContextUpload>,
) -> Result<Json<ContextUploadResponse>> {
    let _content = &req.content;

    Err(ErrorKind::NotImplemented.with_message("context upload not yet implemented"))
}

fn upload_docs(op: TransformOperation) -> TransformOperation {
    op.id("uploadContext")
        .tag("contexts")
        .summary("Upload a context as base64-encoded JSON")
        .description(
            "Accepts a JSON body with base64-encoded content, an optional filename, \
             and an optional content type override.",
        )
}

/// `GET /api/v1/contexts`: list all context identifiers.
#[tracing::instrument(skip_all)]
async fn list(
    State(_state): State<ServiceState>,
) -> Result<Json<ContextListResponse>> {
    Err(ErrorKind::NotImplemented.with_message("context listing not yet implemented"))
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
    State(_state): State<ServiceState>,
    Path(ContentPath { id }): Path<ContentPath>,
) -> Result<Json<ContextDownloadResponse>> {
    Err(ErrorKind::NotImplemented
        .with_message(format!("context download not yet implemented (id: {id})")))
}

fn download_docs(op: TransformOperation) -> TransformOperation {
    op.id("downloadContext")
        .tag("contexts")
        .summary("Download a previously uploaded context")
        .description("Retrieves context data by its UUID, returning base64-encoded bytes.")
}

/// `DELETE /api/v1/contexts/{id}`: delete a single context.
#[tracing::instrument(skip_all, fields(%id))]
async fn delete(
    State(_state): State<ServiceState>,
    Path(ContentPath { id }): Path<ContentPath>,
) -> Result<Json<ContextDeleteResponse>> {
    Err(ErrorKind::NotImplemented
        .with_message(format!("context deletion not yet implemented (id: {id})")))
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
    State(_state): State<ServiceState>,
) -> Result<Json<ContextDeleteAllResponse>> {
    Err(ErrorKind::NotImplemented.with_message("context bulk deletion not yet implemented"))
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
