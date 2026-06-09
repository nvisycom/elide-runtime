//! File upload, download, list, and deletion handlers.
//!
//! | Method   | Path                       | Description                                |
//! |----------|----------------------------|--------------------------------------------|
//! | `POST`   | `/files`                   | Upload file (base64 JSON body)             |
//! | `GET`    | `/files`                   | List uploaded files (paginated metadata)   |
//! | `GET`    | `/files/{id}`              | Get file metadata (JSON)                   |
//! | `GET`    | `/files/{id}/content`      | Download raw bytes (application/octet-stream) |
//! | `DELETE` | `/files/{id}`              | Delete a single file                       |
//! | `DELETE` | `/files`                   | Delete all files                           |
//!
//! Paths are relative — the version prefix (e.g. `/api/v1`) is applied
//! by the version module.

use std::path::PathBuf;

use aide::axum::ApiRouter;
use aide::axum::routing::{delete_with, get_with, post_with};
use aide::transform::TransformOperation;
use axum::body::Body;
use axum::extract::{Query, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use nvisy_document::registry::Registry;
use nvisy_document::{Content, ContentData, ContentDescriptor};

use super::error::Result;
use super::request::{ContentPath, MAX_PAGE_LIMIT, NewFile, Pagination};
use super::response::{FileEntry, FileId, FileList, Page};
use crate::extract::{ActorId, Json, Path};
use crate::middleware::{DEFAULT_READ_TIMEOUT, DEFAULT_WRITE_TIMEOUT, RouterTimeoutExt};
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

    let mut descriptor = match req.filename {
        Some(ref name) => ContentDescriptor::with_path(name),
        None => ContentDescriptor::new(),
    };
    if let Some(ref mime) = req.content_type {
        descriptor.content_type = Some(mime.clone());
    }
    if let Some(ref name) = req.filename {
        descriptor.filename = Some(PathBuf::from(name));
    }

    let content = Content::with_descriptor(content_data, descriptor);
    let id = registry
        .register_content(actor_id, content, Some(&req.annotations))
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

/// `GET /files/{id}`: return file metadata as JSON. File bytes live
/// at `/files/{id}/content`.
#[tracing::instrument(target = TARGET, skip_all, fields(%id, %actor_id))]
async fn get_file_metadata(
    State(registry): State<Registry>,
    ActorId(actor_id): ActorId,
    Path(ContentPath { id }): Path<ContentPath>,
) -> Result<Json<FileEntry>> {
    let handle = registry.read_content(actor_id, id).await?;
    let record = handle.record().await?;

    tracing::debug!(target: TARGET, "file metadata read");

    Ok(Json(FileEntry {
        id,
        content_type: record.content_type().map(String::from),
        filename: record.filename_lossy(),
        size: record.digest.size,
        sha256: record.digest.sha256,
    }))
}

fn get_file_metadata_docs(op: TransformOperation) -> TransformOperation {
    op.id("getFileMetadata")
        .tag("files")
        .summary("Get file metadata")
        .description(
            "Returns metadata for a previously uploaded file. File bytes are served \
             separately by `GET /files/{id}/content`.",
        )
}

/// `GET /files/{id}/content`: stream raw file bytes as
/// `application/octet-stream`.
#[tracing::instrument(target = TARGET, skip_all, fields(%id, %actor_id))]
async fn download_file_content(
    State(registry): State<Registry>,
    ActorId(actor_id): ActorId,
    Path(ContentPath { id }): Path<ContentPath>,
) -> Result<Response> {
    let handle = registry.read_content(actor_id, id).await?;
    let data = handle.content_data().await?;
    let record = handle.record().await?;
    let size = data.size();

    tracing::debug!(target: TARGET, size, "file content downloaded");

    let content_type = record
        .content_type()
        .and_then(|s| HeaderValue::from_str(s).ok())
        .unwrap_or_else(|| HeaderValue::from_static("application/octet-stream"));

    let mut response = (StatusCode::OK, Body::from(data.as_bytes().to_vec())).into_response();
    response
        .headers_mut()
        .insert(header::CONTENT_TYPE, content_type);
    if let Some(name) = record.filename_lossy()
        && let Ok(disposition) = HeaderValue::from_str(&format!("attachment; filename=\"{name}\""))
    {
        response
            .headers_mut()
            .insert(header::CONTENT_DISPOSITION, disposition);
    }
    Ok(response)
}

fn download_file_content_docs(op: TransformOperation) -> TransformOperation {
    op.id("downloadFileContent")
        .tag("files")
        .summary("Download raw file bytes")
        .description(
            "Returns the file's raw bytes with the original content type. \
             Metadata (size, sha256, filename) is available separately at \
             `GET /files/{id}`.",
        )
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
    let limit = pagination.limit.min(MAX_PAGE_LIMIT);
    let paged = registry
        .list_content_with_record(actor_id, pagination.offset, limit)
        .await?;
    let page = Page::from_paged(paged, &pagination, |(id, record)| FileEntry {
        id,
        content_type: record.content_type().map(String::from),
        filename: record.filename_lossy(),
        size: record.digest.size,
        sha256: record.digest.sha256,
    });
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
        .api_route(
            "/files/{id}",
            get_with(get_file_metadata, get_file_metadata_docs),
        )
        .api_route(
            "/files/{id}/content",
            get_with(download_file_content, download_file_content_docs),
        )
        .with_timeout(DEFAULT_READ_TIMEOUT);

    let write_routes = ApiRouter::new()
        .api_route(
            "/files",
            post_with(upload_file, upload_file_docs)
                .delete_with(delete_all_files, delete_all_files_docs),
        )
        .api_route("/files/{id}", delete_with(delete_file, delete_file_docs))
        .with_timeout(DEFAULT_WRITE_TIMEOUT);

    read_routes.merge(write_routes)
}
