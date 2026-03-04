//! File upload, download, list, and deletion handlers.
//!
//! # Endpoints
//!
//! | Method   | Path                    | Description                         |
//! |----------|-------------------------|-------------------------------------|
//! | `POST`   | `/api/v1/files`         | Upload file (base64 JSON)           |
//! | `GET`    | `/api/v1/files`         | List all uploaded file IDs          |
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
use nvisy_registry::ActorId;

use super::error::{ErrorKind, Result};
use super::request::{ContentPath, FileUpload};
use super::response::{
    FileDeleteAllResponse, FileDeleteResponse, FileDownloadResponse, FileListResponse,
    FileUploadResponse,
};
use crate::extract::{Json, Path};
use crate::service::ServiceState;

/// Resolves the actor from the request, falling back to a nil UUID.
fn resolve_actor(actor_id: Option<uuid::Uuid>) -> ActorId {
    actor_id.map(ActorId::from).unwrap_or_else(|| ActorId::from(uuid::Uuid::nil()))
}

/// `POST /api/v1/files`: upload a file as base64-encoded JSON.
#[tracing::instrument(skip_all, fields(filename = req.filename.as_deref()))]
async fn upload(
    State(state): State<ServiceState>,
    Json(req): Json<FileUpload>,
) -> Result<(StatusCode, Json<FileUploadResponse>)> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(&req.content)
        .map_err(|e| ErrorKind::BadRequest.with_message(format!("invalid base64: {e}")))?;

    let actor = resolve_actor(req.actor_id);
    let size = bytes.len();
    let mut content_data = ContentData::from(bytes);
    if let Some(mime) = req.content_type {
        content_data.mime = Some(mime);
    }
    let content = Content::new(content_data);
    let handle = state.registry().register_content(actor, content).await?;
    let id = handle.content_source().as_uuid();

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
    State(state): State<ServiceState>,
    Path(ContentPath { id }): Path<ContentPath>,
) -> Result<Json<FileDownloadResponse>> {
    let actor = resolve_actor(None);
    let handle = state.registry().read_content(actor, id).await?;
    let content_data = handle.content_data().await?;
    let content = base64::engine::general_purpose::STANDARD.encode(content_data.as_bytes());
    Ok(Json(FileDownloadResponse { id, content }))
}

fn download_docs(op: TransformOperation) -> TransformOperation {
    op.id("downloadFile")
        .tag("files")
        .summary("Download a previously uploaded file")
        .description("Retrieves file content by its UUID, returning base64-encoded bytes.")
}

/// `GET /api/v1/files`: list all uploaded file IDs.
#[tracing::instrument(skip_all)]
async fn list(
    State(state): State<ServiceState>,
) -> Result<Json<FileListResponse>> {
    let actor = resolve_actor(None);
    let files = state.registry().list_content(actor).await?;
    Ok(Json(FileListResponse { files }))
}

fn list_docs(op: TransformOperation) -> TransformOperation {
    op.id("listFiles")
        .tag("files")
        .summary("List all uploaded file IDs")
        .description("Returns a list of UUIDs for all files currently stored in the registry.")
}

/// `DELETE /api/v1/files/{id}`: delete a single uploaded file.
#[tracing::instrument(skip_all, fields(%id))]
async fn delete(
    State(state): State<ServiceState>,
    Path(ContentPath { id }): Path<ContentPath>,
) -> Result<Json<FileDeleteResponse>> {
    let actor = resolve_actor(None);
    state.registry().unregister_content(actor, id).await?;
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
    let actor = resolve_actor(None);
    let deleted = state.registry().unregister_all_content(actor).await?;
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
            post_with(upload, upload_docs)
                .get_with(list, list_docs)
                .delete_with(delete_all, delete_all_docs),
        )
        .api_route(
            "/api/v1/files/{id}",
            get_with(download, download_docs).delete_with(delete, delete_docs),
        )
}
