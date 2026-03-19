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
use nvisy_core::content::{Content, ContentData, ContentMetadata};
use nvisy_registry::Registry;

use super::error::Result;
use super::request::{ContentPath, NewFile};
use super::response::{File, FileId, FileList};
use super::utility::Base64;
use crate::extract::{ActorId, Json, Path};
use crate::service::ServiceState;

const TARGET: &str = "nvisy_server::files";

/// `POST /api/v1/files`: upload a file as base64-encoded JSON.
#[tracing::instrument(
    target = "nvisy_server::files",
    skip_all,
    fields(%actor_id, filename = req.filename.as_deref()),
)]
async fn upload_file(
    State(registry): State<Registry>,
    ActorId(actor_id): ActorId,
    Json(req): Json<NewFile>,
) -> Result<(StatusCode, Json<FileId>)> {
    let bytes = req.content.decode()?;
    let size = bytes.len();

    let mut content_data = ContentData::from(bytes);
    if let Some(ref mime) = req.content_type {
        content_data = content_data.with_content_type(mime.clone());
    }
    if let Some(ref name) = req.filename {
        content_data = content_data.with_filename(name.as_str());
    }

    let mut metadata = match req.filename {
        Some(ref name) => ContentMetadata::with_path(name),
        None => ContentMetadata::new(),
    };
    if let Some(ref mime) = req.content_type {
        metadata.set_extra("content_type", serde_json::Value::String(mime.clone()));
    }

    let content = Content::with_metadata(content_data, metadata);
    let handle = registry.register_content(actor_id, content).await?;
    let id = handle.content_source().as_uuid();

    tracing::info!(
        target: TARGET,
        %id,
        size,
        content_type = req.content_type.as_deref().unwrap_or("<none>"),
        "file uploaded",
    );

    Ok((StatusCode::CREATED, Json(FileId { id })))
}

fn upload_file_docs(op: TransformOperation) -> TransformOperation {
    op.id("uploadFile")
        .tag("files")
        .summary("Upload a file as base64-encoded JSON")
        .description(
            "Accepts a JSON body with base64-encoded content, an optional filename, \
             and an optional content type override.",
        )
}

/// `GET /api/v1/files/{id}`: download previously uploaded content.
#[tracing::instrument(
    target = "nvisy_server::files",
    skip_all,
    fields(%id, %actor_id),
)]
async fn download_file(
    State(registry): State<Registry>,
    ActorId(actor_id): ActorId,
    Path(ContentPath { id }): Path<ContentPath>,
) -> Result<Json<File>> {
    let handle = registry.read_content(actor_id, id).await?;
    let content_data = handle.content_data().await?;
    let metadata = handle.metadata().await?;

    tracing::debug!(target: TARGET, size = content_data.size(), "file downloaded");

    Ok(Json(File {
        id,
        content: Base64::encode(content_data.as_bytes()),
        content_type: metadata
            .get_extra("content_type")
            .and_then(|v| v.as_str())
            .map(String::from),
        filename: metadata.filename().map(String::from),
    }))
}

fn download_file_docs(op: TransformOperation) -> TransformOperation {
    op.id("downloadFile")
        .tag("files")
        .summary("Download a previously uploaded file")
        .description("Retrieves file content by its UUID, returning base64-encoded bytes.")
}

/// `GET /api/v1/files`: list all uploaded file IDs.
#[tracing::instrument(
    target = "nvisy_server::files",
    skip_all,
    fields(%actor_id),
)]
async fn list_files(
    State(registry): State<Registry>,
    ActorId(actor_id): ActorId,
) -> Result<Json<FileList>> {
    let files = registry.list_content(actor_id).await?;
    tracing::debug!(target: TARGET, count = files.len(), "files listed");
    Ok(Json(FileList { files }))
}

fn list_files_docs(op: TransformOperation) -> TransformOperation {
    op.id("listFiles")
        .tag("files")
        .summary("List all uploaded file IDs")
        .description("Returns a list of UUIDs for all files currently stored in the registry.")
}

/// `DELETE /api/v1/files/{id}`: delete a single uploaded file.
#[tracing::instrument(
    target = "nvisy_server::files",
    skip_all,
    fields(%id, %actor_id),
)]
async fn delete_file(
    State(registry): State<Registry>,
    ActorId(actor_id): ActorId,
    Path(ContentPath { id }): Path<ContentPath>,
) -> Result<StatusCode> {
    registry.unregister_content(actor_id, id).await?;
    tracing::info!(target: TARGET, "file deleted");
    Ok(StatusCode::NO_CONTENT)
}

fn delete_file_docs(op: TransformOperation) -> TransformOperation {
    op.id("deleteFile")
        .tag("files")
        .summary("Delete an uploaded file")
        .description("Removes a single file identified by its UUID.")
}

/// `DELETE /api/v1/files`: delete all uploaded files.
#[tracing::instrument(
    target = "nvisy_server::files",
    skip_all,
    fields(%actor_id),
)]
async fn delete_all_files(
    State(registry): State<Registry>,
    ActorId(actor_id): ActorId,
) -> Result<StatusCode> {
    let deleted = registry.unregister_all_content(actor_id).await?;
    tracing::info!(target: TARGET, deleted, "all files deleted");
    Ok(StatusCode::NO_CONTENT)
}

fn delete_all_files_docs(op: TransformOperation) -> TransformOperation {
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
            post_with(upload_file, upload_file_docs)
                .get_with(list_files, list_files_docs)
                .delete_with(delete_all_files, delete_all_files_docs),
        )
        .api_route(
            "/api/v1/files/{id}",
            get_with(download_file, download_file_docs).delete_with(delete_file, delete_file_docs),
        )
}
