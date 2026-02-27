//! Content ingestion handlers: upload, download, and deletion.
//!
//! # Endpoints
//!
//! | Method   | Path                     | Description                         |
//! |----------|--------------------------|-------------------------------------|
//! | `POST`   | `/api/v1/ingest`         | Upload content (multipart)          |
//! | `GET`    | `/api/v1/ingest/{id}`    | Download previously uploaded content|
//! | `DELETE` | `/api/v1/ingest/{id}`    | Delete a single content item        |
//! | `DELETE` | `/api/v1/ingest`         | Delete all content items            |
//!
//! # Upload format
//!
//! The upload endpoint accepts `multipart/form-data` with the following fields:
//!
//! | Field          | Kind   | Required | Description                              |
//! |----------------|--------|----------|------------------------------------------|
//! | `file`         | file   | yes      | Binary content to ingest                 |
//! | `content_type` | text   | no       | MIME type override (e.g. `text/csv`)     |
//!
//! The MIME type is resolved in the following order:
//! 1. Explicit `content_type` text field (if present).
//! 2. `Content-Type` header of the `file` part.
//! 3. Downstream detection via magic bytes / filename heuristics.

use aide::axum::ApiRouter;
use aide::axum::routing::{delete_with, get_with};
use aide::transform::TransformOperation;
use axum::extract::State;
use nvisy_core::io::{Content, ContentData};
use uuid::Uuid;

use super::error::{ErrorKind, Result};
use super::response::{DeleteAllResponse, DeleteResponse, DownloadResponse, UploadResponse};
use crate::extract::{Json, Path, Upload};
use crate::service::ServiceState;

/// `POST /api/v1/ingest`: upload content as multipart form data.
///
/// Expects a multipart form with a `file` field containing the binary content.
/// An optional `content_type` text field may override the detected MIME type.
#[tracing::instrument(skip_all)]
async fn upload(
    State(state): State<ServiceState>,
    upload: Upload,
) -> Result<Json<UploadResponse>> {
    let size = upload.bytes.len();
    let mut content_data = ContentData::from(upload.bytes);
    if let Some(mime) = upload.content_type {
        content_data.mime = Some(mime);
    }
    let content = Content::new(content_data);
    let handler = state.content_registry().register(content).await?;
    let id = handler.content_source().as_uuid();

    tracing::info!(
        %id,
        size,
        filename = upload.filename.as_deref().unwrap_or("<none>"),
        "content uploaded",
    );

    Ok(Json(UploadResponse { id }))
}

/// `GET /api/v1/ingest/{id}`: download previously uploaded content.
///
/// Returns the content as base64-encoded bytes along with its identifier.
/// Currently unimplemented: returns a 501 error.
#[tracing::instrument(skip_all, fields(%id))]
async fn download(
    State(_state): State<ServiceState>,
    Path(id): Path<Uuid>,
) -> Result<Json<DownloadResponse>> {
    Err(ErrorKind::NotImplemented
        .with_message(format!("content download not yet implemented (id: {id})")))
}

fn download_docs(op: TransformOperation) -> TransformOperation {
    op.id("downloadContent")
        .tag("ingest")
        .summary("Download previously uploaded content")
        .description("Retrieves content by its UUID, returning base64-encoded bytes.")
}

/// `DELETE /api/v1/ingest/{id}`: delete a single uploaded content item.
///
/// Removes the content directory identified by the given UUID from the
/// registry. Returns the deleted identifier on success.
#[tracing::instrument(skip_all, fields(%id))]
async fn delete(
    State(state): State<ServiceState>,
    Path(id): Path<Uuid>,
) -> Result<Json<DeleteResponse>> {
    state.content_registry().delete(id).await?;
    tracing::info!(%id, "content deleted");
    Ok(Json(DeleteResponse { id }))
}

fn delete_docs(op: TransformOperation) -> TransformOperation {
    op.id("deleteContent")
        .tag("ingest")
        .summary("Delete uploaded content")
        .description("Removes a single content item identified by its UUID.")
}

/// `DELETE /api/v1/ingest`: delete all uploaded content.
///
/// Removes every content directory under the registry's base path.
/// Returns the number of items deleted.
#[tracing::instrument(skip_all)]
async fn delete_all(
    State(state): State<ServiceState>,
) -> Result<Json<DeleteAllResponse>> {
    let deleted = state.content_registry().delete_all().await?;
    tracing::info!(deleted, "all content deleted");
    Ok(Json(DeleteAllResponse { deleted }))
}

fn delete_all_docs(op: TransformOperation) -> TransformOperation {
    op.id("deleteAllContent")
        .tag("ingest")
        .summary("Delete all uploaded content")
        .description("Removes every content item currently stored in the registry.")
}

/// Ingest routes.
pub fn routes() -> ApiRouter<ServiceState> {
    ApiRouter::new()
        .route("/api/v1/ingest", axum::routing::post(upload))
        .api_route(
            "/api/v1/ingest",
            delete_with(delete_all, delete_all_docs),
        )
        .api_route(
            "/api/v1/ingest/{id}",
            get_with(download, download_docs).delete_with(delete, delete_docs),
        )
}
