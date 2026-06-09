//! File upload, download, list, and deletion handlers.
//!
//! | Method   | Path                              | Description                                |
//! |----------|-----------------------------------|--------------------------------------------|
//! | `POST`   | `/files`                          | Upload raw bytes (octet-stream)            |
//! | `GET`    | `/files`                          | List uploaded files (paginated metadata)   |
//! | `GET`    | `/files/{id}`                     | Get file metadata (descriptor + digest)    |
//! | `GET`    | `/files/{id}/content`             | Download raw bytes (octet-stream)          |
//! | `GET`    | `/files/{id}/annotations`         | Read the annotation overlay                |
//! | `PUT`    | `/files/{id}/annotations`         | Replace the annotation overlay             |
//! | `DELETE` | `/files/{id}`                     | Delete a single file (cascades annotations)|
//! | `DELETE` | `/files`                          | Delete all files                           |
//!
//! Paths are relative — the version prefix (e.g. `/api/v1`) is
//! applied by the version module.

use aide::axum::ApiRouter;
use aide::axum::routing::{delete_with, get_with, post_with, put_with};
use aide::transform::TransformOperation;
use axum::body::{Body, Bytes};
use axum::extract::{Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use nvisy_engine::document::AnyAnnotations;
use nvisy_engine::registry::Registry;
use nvisy_engine::{Content, ContentData, ContentDescriptor};

use super::error::Result;
use super::request::{ContentPath, MAX_PAGE_LIMIT, Pagination};
use super::response::{FileId, FileList, FileMetadata, Page};
use crate::extract::{ActorId, Json, Path};
use crate::middleware::{DEFAULT_READ_TIMEOUT, DEFAULT_WRITE_TIMEOUT, RouterTimeoutExt};
use crate::service::ServiceState;

const TARGET: &str = "nvisy_server::files";

#[tracing::instrument(target = TARGET, skip_all, fields(%actor_id))]
async fn upload_file(
    State(registry): State<Registry>,
    ActorId(actor_id): ActorId,
    headers: HeaderMap,
    body: Bytes,
) -> Result<(StatusCode, Json<FileId>)> {
    let size = body.len();
    let content_data = ContentData::from(body);

    let mut descriptor = ContentDescriptor::new();
    if let Some(mime) = headers
        .get(header::CONTENT_TYPE)
        .and_then(|h| h.to_str().ok())
    {
        descriptor.content_type = Some(mime.to_owned());
    }
    if let Some(name) = headers
        .get(header::CONTENT_DISPOSITION)
        .and_then(|h| h.to_str().ok())
        .and_then(parse_disposition_filename)
    {
        descriptor.filename = Some(name.into());
    }

    let content = Content::with_descriptor(content_data, descriptor);
    let id = registry
        .register_content(actor_id, content, None)
        .await?
        .content_source()
        .as_uuid();

    tracing::info!(target: TARGET, %id, size, "file uploaded");
    Ok((StatusCode::CREATED, Json(FileId { id })))
}

fn upload_file_docs(op: TransformOperation) -> TransformOperation {
    op.id("uploadFile")
        .tag("files")
        .summary("Upload a file")
        .description(
            "Accepts raw bytes as the request body. `Content-Type` carries the \
             caller-supplied MIME hint; `Content-Disposition` carries the original \
             filename (e.g. `attachment; filename=\"scan.pdf\"`). Annotations are \
             attached separately via `PUT /files/{id}/annotations`.",
        )
}

#[tracing::instrument(target = TARGET, skip_all, fields(%id, %actor_id))]
async fn get_file_metadata(
    State(registry): State<Registry>,
    ActorId(actor_id): ActorId,
    Path(ContentPath { id }): Path<ContentPath>,
) -> Result<Json<FileMetadata>> {
    let handle = registry.read_content(actor_id, id).await?;
    let record = handle.record().await?;
    Ok(Json(FileMetadata {
        id,
        descriptor: record.descriptor,
        digest: record.digest,
    }))
}

fn get_file_metadata_docs(op: TransformOperation) -> TransformOperation {
    op.id("getFileMetadata")
        .tag("files")
        .summary("Get file metadata")
        .description(
            "Returns the file's caller-supplied descriptor + registry-computed \
             digest. Annotations live at `GET /files/{id}/annotations`; bytes at \
             `GET /files/{id}/content`.",
        )
}

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
            "Returns the file's raw bytes. `Content-Type` carries the best-available \
             MIME (caller-supplied, then sniffed); `Content-Disposition` carries the \
             original filename when known.",
        )
}

#[tracing::instrument(target = TARGET, skip_all, fields(%id, %actor_id))]
async fn get_file_annotations(
    State(registry): State<Registry>,
    ActorId(actor_id): ActorId,
    Path(ContentPath { id }): Path<ContentPath>,
) -> Result<Json<AnyAnnotations>> {
    let annotations = registry.load_annotations(actor_id, id).await?;
    Ok(Json(annotations))
}

fn get_file_annotations_docs(op: TransformOperation) -> TransformOperation {
    op.id("getFileAnnotations")
        .tag("files")
        .summary("Read the file's annotation overlay")
        .description(
            "Returns the per-modality annotation buckets (`text`, `tabular`, \
             `image`, `audio`) plus document-level `labels`. Returns an empty \
             overlay when nothing has been set.",
        )
}

#[tracing::instrument(target = TARGET, skip_all, fields(%id, %actor_id))]
async fn put_file_annotations(
    State(registry): State<Registry>,
    ActorId(actor_id): ActorId,
    Path(ContentPath { id }): Path<ContentPath>,
    Json(annotations): Json<AnyAnnotations>,
) -> Result<StatusCode> {
    registry
        .store_annotations(actor_id, id, &annotations)
        .await?;
    tracing::info!(target: TARGET, "file annotations updated");
    Ok(StatusCode::NO_CONTENT)
}

fn put_file_annotations_docs(op: TransformOperation) -> TransformOperation {
    op.id("putFileAnnotations")
        .tag("files")
        .summary("Replace the file's annotation overlay")
        .description(
            "Replaces the stored annotations with the request body. Idempotent: \
             PUT with an empty object clears all hints. Annotations attached \
             here are picked up by every subsequent detection / redaction pass \
             that references this content.",
        )
}

#[tracing::instrument(target = TARGET, skip_all, fields(%actor_id))]
async fn list_files(
    State(registry): State<Registry>,
    ActorId(actor_id): ActorId,
    Query(pagination): Query<Pagination>,
) -> Result<Json<FileList>> {
    let limit = pagination.limit.min(MAX_PAGE_LIMIT);
    let paged = registry
        .list_content_with_record(actor_id, pagination.offset, limit)
        .await?;
    let page = Page::from_paged(paged, &pagination, |(id, record)| FileMetadata {
        id,
        descriptor: record.descriptor,
        digest: record.digest,
    });
    tracing::debug!(target: TARGET, total = page.total, count = page.items.len(), "files listed");
    Ok(Json(page))
}

fn list_files_docs(op: TransformOperation) -> TransformOperation {
    op.id("listFiles")
        .tag("files")
        .summary("List stored files")
        .description(
            "Paginated metadata listing. Each entry has the same shape as \
             `GET /files/{id}`. Annotations are not included.",
        )
}

#[tracing::instrument(target = TARGET, skip_all, fields(%id, %actor_id))]
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
        .description("Removes the file and cascades to its annotations.")
}

#[tracing::instrument(target = TARGET, skip_all, fields(%actor_id))]
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
        .summary("Delete every file for the actor")
        .description("Removes every file plus its annotations.")
}

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
        .api_route(
            "/files/{id}/annotations",
            get_with(get_file_annotations, get_file_annotations_docs),
        )
        .with_timeout(DEFAULT_READ_TIMEOUT);

    let write_routes = ApiRouter::new()
        .api_route(
            "/files",
            post_with(upload_file, upload_file_docs)
                .delete_with(delete_all_files, delete_all_files_docs),
        )
        .api_route("/files/{id}", delete_with(delete_file, delete_file_docs))
        .api_route(
            "/files/{id}/annotations",
            put_with(put_file_annotations, put_file_annotations_docs),
        )
        .with_timeout(DEFAULT_WRITE_TIMEOUT);

    read_routes.merge(write_routes)
}

/// Parse the `filename="..."` value out of a `Content-Disposition`
/// header. Accepts both quoted and unquoted forms, returns `None`
/// when the header doesn't carry one.
fn parse_disposition_filename(header_value: &str) -> Option<String> {
    for part in header_value.split(';').map(str::trim) {
        if let Some(rest) = part
            .strip_prefix("filename=")
            .or_else(|| part.strip_prefix("filename*="))
        {
            let unquoted = rest.trim_matches('"');
            if !unquoted.is_empty() {
                return Some(unquoted.to_owned());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::parse_disposition_filename;

    #[test]
    fn quoted_filename() {
        assert_eq!(
            parse_disposition_filename("attachment; filename=\"scan.pdf\""),
            Some("scan.pdf".to_owned())
        );
    }

    #[test]
    fn unquoted_filename() {
        assert_eq!(
            parse_disposition_filename("attachment; filename=scan.pdf"),
            Some("scan.pdf".to_owned())
        );
    }

    #[test]
    fn no_filename() {
        assert_eq!(parse_disposition_filename("attachment"), None);
        assert_eq!(parse_disposition_filename("inline"), None);
    }

    #[test]
    fn empty_filename() {
        assert_eq!(
            parse_disposition_filename("attachment; filename=\"\""),
            None
        );
    }
}
