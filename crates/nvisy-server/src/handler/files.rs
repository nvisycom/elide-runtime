//! File upload, download, and deletion handlers.
//!
//! # Endpoints
//!
//! | Method   | Path                    | Description                         |
//! |----------|-------------------------|-------------------------------------|
//! | `POST`   | `/api/v1/files`         | Upload file (base64 JSON)           |
//! | `GET`    | `/api/v1/files/{id}`    | Download previously uploaded file   |
//! | `DELETE` | `/api/v1/files/{id}`    | Delete a single file                |
//! | `DELETE` | `/api/v1/files`         | Delete all files                    |

use aide::axum::ApiRouter;
use aide::axum::routing::{get_with, post_with};
use aide::transform::TransformOperation;
use axum::extract::State;
use axum::http::StatusCode;
use base64::Engine as _;
use nvisy_core::io::{Content, ContentData};

use super::error::{ErrorKind, Result};
use super::request::{ContentPath, FileUpload};
use super::response::{
    FileDeleteAllResponse, FileDeleteResponse, FileDownloadResponse, FileUploadResponse,
};
use crate::extract::{Json, Path};
use crate::service::ServiceState;

/// `POST /api/v1/files`: upload a file as base64-encoded JSON.
#[tracing::instrument(skip_all, fields(filename = req.filename.as_deref()))]
async fn upload(
    State(state): State<ServiceState>,
    Json(req): Json<FileUpload>,
) -> Result<(StatusCode, Json<FileUploadResponse>)> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(&req.content)
        .map_err(|e| ErrorKind::BadRequest.with_message(format!("invalid base64: {e}")))?;

    let size = bytes.len();
    let mut content_data = ContentData::from(bytes);
    if let Some(mime) = req.content_type {
        content_data.mime = Some(mime);
    }
    let content = Content::new(content_data);
    let handler = state.content_registry().register(content).await?;
    let id = handler.content_source().as_uuid();

    tracing::info!(
        %id,
        size,
        filename = req.filename.as_deref().unwrap_or("<none>"),
        "file uploaded",
    );

    Ok((StatusCode::CREATED, Json(FileUploadResponse { id })))
}

fn upload_docs(op: TransformOperation) -> TransformOperation {
    op.id("uploadFile")
        .tag("files")
        .summary("Upload a file as base64-encoded JSON")
        .description(
            "Accepts a JSON body with base64-encoded content, an optional filename, \
             and an optional content type override.",
        )
}

/// `GET /api/v1/files/{id}`: download previously uploaded content.
#[tracing::instrument(skip_all, fields(%id))]
async fn download(
    State(_state): State<ServiceState>,
    Path(ContentPath { id }): Path<ContentPath>,
) -> Result<Json<FileDownloadResponse>> {
    Err(ErrorKind::NotImplemented
        .with_message(format!("file download not yet implemented (id: {id})")))
}

fn download_docs(op: TransformOperation) -> TransformOperation {
    op.id("downloadFile")
        .tag("files")
        .summary("Download a previously uploaded file")
        .description("Retrieves file content by its UUID, returning base64-encoded bytes.")
}

/// `DELETE /api/v1/files/{id}`: delete a single uploaded file.
#[tracing::instrument(skip_all, fields(%id))]
async fn delete(
    State(state): State<ServiceState>,
    Path(ContentPath { id }): Path<ContentPath>,
) -> Result<Json<FileDeleteResponse>> {
    state.content_registry().delete(id).await?;
    tracing::info!(%id, "file deleted");
    Ok(Json(FileDeleteResponse { id }))
}

fn delete_docs(op: TransformOperation) -> TransformOperation {
    op.id("deleteFile")
        .tag("files")
        .summary("Delete an uploaded file")
        .description("Removes a single file identified by its UUID.")
}

/// `DELETE /api/v1/files`: delete all uploaded files.
#[tracing::instrument(skip_all)]
async fn delete_all(
    State(state): State<ServiceState>,
) -> Result<Json<FileDeleteAllResponse>> {
    let deleted = state.content_registry().delete_all().await?;
    tracing::info!(deleted, "all files deleted");
    Ok(Json(FileDeleteAllResponse { deleted }))
}

fn delete_all_docs(op: TransformOperation) -> TransformOperation {
    op.id("deleteAllFiles")
        .tag("files")
        .summary("Delete all uploaded files")
        .description("Removes every file currently stored in the registry.")
}

/// File routes.
pub fn routes() -> ApiRouter<ServiceState> {
    ApiRouter::new()
        .api_route(
            "/api/v1/files",
            post_with(upload, upload_docs).delete_with(delete_all, delete_all_docs),
        )
        .api_route(
            "/api/v1/files/{id}",
            get_with(download, download_docs).delete_with(delete, delete_docs),
        )
}
