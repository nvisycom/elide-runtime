//! File upload, download, list, and deletion handlers.
//!
//! # Endpoints
//!
//! | Method   | Path                        | Description                         |
//! |----------|-----------------------------|-------------------------------------|
//! | `POST`   | `/files`             | Upload file (base64 JSON)           |
//! | `GET`    | `/files`             | List all uploaded file IDs          |
//! | `GET`    | `/files/{id}`        | Download previously uploaded file   |
//! | `DELETE` | `/files/{id}`        | Delete a single file                |
//! | `DELETE` | `/files`             | Delete all files                    |
//!
//! Paths are relative — the version prefix (e.g. `/api/v1`) is applied
//! by the version module.

use aide::axum::ApiRouter;
use aide::axum::routing::{get_with, post_with};
use aide::transform::TransformOperation;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use nvisy_core::content::{Content, ContentData, ContentMetadata};
use nvisy_engine::registry::Registry;

use super::error::Result;
use super::request::{ContentPath, NewFile, Pagination};
use super::response::{File, FileEntry, FileId, FileList};
use super::utility::Base64;
use crate::extract::{ActorId, Json, Path};
use crate::middleware::{DEFAULT_READ_TIMEOUT_SECS, DEFAULT_WRITE_TIMEOUT_SECS, RouterTimeoutExt};
use crate::service::ServiceState;

const TARGET: &str = "nvisy_server::files";

/// `POST /files`: upload a file as base64-encoded JSON.
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

    let content_data = ContentData::from(bytes);

    let mut metadata = match req.filename {
        Some(ref name) => ContentMetadata::with_path(name),
        None => ContentMetadata::new(),
    };
    if let Some(ref mime) = req.content_type {
        metadata.content_type = Some(mime.clone());
    }
    if let Some(ref name) = req.filename {
        metadata.filename = Some(std::path::PathBuf::from(name));
    }

    let content = Content::with_metadata(content_data, metadata);
    let id = registry
        .register_content(actor_id, content)
        .await?
        .content_source()
        .as_uuid();

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

/// `GET /files/{id}`: download previously uploaded content.
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
    let data = handle.content_data().await?;
    let metadata = handle.metadata().await?;
    let content = Content::with_metadata(data, metadata);

    tracing::debug!(target: TARGET, size = content.size(), "file downloaded");

    let meta = content.metadata();
    Ok(Json(File {
        id,
        content: Base64::encode(content.as_bytes()),
        content_type: meta.and_then(|m| m.content_type()).map(String::from),
        filename: content.filename().map(|p| p.to_string_lossy().to_string()),
        size: content.size() as u64,
        sha256: content.data().sha256_hex(),
    }))
}

fn download_file_docs(op: TransformOperation) -> TransformOperation {
    op.id("downloadFile")
        .tag("files")
        .summary("Download a previously uploaded file")
        .description("Retrieves file content by its UUID, returning base64-encoded bytes.")
}

/// `GET /files`: list all uploaded file IDs.
#[tracing::instrument(
    target = "nvisy_server::files",
    skip_all,
    fields(%actor_id),
)]
async fn list_files(
    State(registry): State<Registry>,
    ActorId(actor_id): ActorId,
    Query(pagination): Query<Pagination>,
) -> Result<Json<FileList>> {
    let entries = registry.list_content_with_metadata(actor_id).await?;
    let summaries: Vec<FileEntry> = entries
        .into_iter()
        .map(|(id, meta)| FileEntry {
            id,
            content_type: meta.content_type().map(String::from),
            filename: meta.filename.map(|p| p.to_string_lossy().to_string()),
            size: meta.size,
            sha256: meta.sha256,
        })
        .collect();
    let page = pagination.paginate(summaries);
    tracing::debug!(target: TARGET, total = page.total, count = page.items.len(), "files listed");
    Ok(Json(page))
}

fn list_files_docs(op: TransformOperation) -> TransformOperation {
    op.id("listFiles")
        .tag("files")
        .summary("List all uploaded file IDs")
        .description("Returns a list of UUIDs for all files currently stored in the registry.")
}

/// `DELETE /files/{id}`: delete a single uploaded file.
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

/// `DELETE /files`: delete all uploaded files.
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

/// File routes for API v1 (relative paths).
pub fn routes_v1() -> ApiRouter<ServiceState> {
    let read_routes = ApiRouter::new()
        .api_route("/files", get_with(list_files, list_files_docs))
        .api_route("/files/{id}", get_with(download_file, download_file_docs))
        .with_timeout(DEFAULT_READ_TIMEOUT_SECS);

    let write_routes = ApiRouter::new()
        .api_route(
            "/files",
            post_with(upload_file, upload_file_docs)
                .delete_with(delete_all_files, delete_all_files_docs),
        )
        .api_route(
            "/files/{id}",
            aide::axum::routing::delete_with(delete_file, delete_file_docs),
        )
        .with_timeout(DEFAULT_WRITE_TIMEOUT_SECS);

    read_routes.merge(write_routes)
}
