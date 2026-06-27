//! File upload / download / list / delete handlers.

use std::collections::HashMap;

use aide::axum::ApiRouter;
use aide::axum::routing::{delete_with, get_with, post_with};
use aide::transform::TransformOperation;
use axum::body::{Body, Bytes};
use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::Response;
use nvisy_engine::{Engine, FileDescriptor, FileRegistry};

use super::error::{ErrorKind, Result};
use super::request::{FilePath, FileQuery};
use super::response::{FileId, FileMetadataResponse, Page};
use crate::extract::{ActorId, Json, Path};
use crate::middleware::{DEFAULT_READ_TIMEOUT, DEFAULT_WRITE_TIMEOUT, RouterTimeoutExt};
use crate::service::ServiceState;

const TARGET: &str = "nvisy_server::files";

#[tracing::instrument(target = TARGET, skip_all, fields(%actor_id))]
async fn upload_file(
    State(engine): State<Engine>,
    ActorId(actor_id): ActorId,
    headers: HeaderMap,
    body: Bytes,
) -> Result<(StatusCode, Json<FileId>)> {
    let size = body.len();
    let filename = parse_filename(&headers);
    let content_type = headers
        .get(header::CONTENT_TYPE)
        .and_then(|h| h.to_str().ok())
        .map(|s| s.to_owned().into());
    let extension = filename
        .as_deref()
        .and_then(filename_extension)
        .ok_or_else(|| {
            ErrorKind::BadRequest.with_message(
                "cannot derive file extension; supply a `Content-Disposition: \
                 attachment; filename=\"…\"` header with an extension",
            )
        })?
        .to_owned()
        .into();

    let descriptor = FileDescriptor {
        filename: filename.map(Into::into),
        content_type,
        extension,
        lineage: None,
        descriptor_labels: Vec::new(),
        descriptor_metadata: HashMap::new(),
    };

    let metadata = engine
        .registry()
        .put_file(actor_id, descriptor, body)
        .await?;
    tracing::info!(target: TARGET, id = %metadata.id, size, "file uploaded");
    Ok((StatusCode::CREATED, Json(FileId { id: metadata.id })))
}

fn upload_file_docs(op: TransformOperation) -> TransformOperation {
    op.id("uploadFile")
        .tag("files")
        .summary("Upload a file")
        .description(
            "Accepts raw bytes as the request body. `Content-Type` carries the \
             caller-supplied MIME hint; `Content-Disposition` carries the original \
             filename (e.g. `attachment; filename=\"scan.pdf\"`). The extension \
             the codec resolves on is derived from the filename.",
        )
}

#[tracing::instrument(target = TARGET, skip_all, fields(%id, %actor_id))]
async fn get_file(
    State(engine): State<Engine>,
    ActorId(actor_id): ActorId,
    Path(FilePath { id }): Path<FilePath>,
) -> Result<Json<FileMetadataResponse>> {
    let metadata = engine.registry().get_file(actor_id, id).await?;
    Ok(Json(metadata))
}

fn get_file_docs(op: TransformOperation) -> TransformOperation {
    op.id("getFile").tag("files").summary("Get file metadata")
}

#[tracing::instrument(target = TARGET, skip_all, fields(%id, %actor_id))]
async fn get_file_content(
    State(engine): State<Engine>,
    ActorId(actor_id): ActorId,
    Path(FilePath { id }): Path<FilePath>,
) -> Result<Response> {
    let metadata = engine.registry().get_file(actor_id, id).await?;
    let bytes = engine.registry().get_file_bytes(actor_id, id).await?;

    let mut response = Response::new(Body::from(bytes));
    if let Some(ct) = metadata.content_type.as_ref()
        && let Ok(value) = ct.as_str().parse()
    {
        response.headers_mut().insert(header::CONTENT_TYPE, value);
    }
    if let Some(name) = metadata.filename.as_ref() {
        let disposition = format!("attachment; filename=\"{}\"", name.as_str());
        if let Ok(value) = disposition.parse() {
            response
                .headers_mut()
                .insert(header::CONTENT_DISPOSITION, value);
        }
    }
    Ok(response)
}

fn get_file_content_docs(op: TransformOperation) -> TransformOperation {
    op.id("getFileContent")
        .tag("files")
        .summary("Download file bytes")
        .description(
            "Returns the raw bytes of the file with the upload's `Content-Type` \
             and a `Content-Disposition` carrying the original filename.",
        )
}

#[tracing::instrument(target = TARGET, skip_all, fields(%actor_id))]
async fn list_files(
    State(engine): State<Engine>,
    ActorId(actor_id): ActorId,
    Query(query): Query<FileQuery>,
) -> Result<Json<Page<FileMetadataResponse>>> {
    let items = engine.registry().list_files(actor_id).await?;
    Ok(Json(Page::paginate(items, &query.pagination)))
}

fn list_files_docs(op: TransformOperation) -> TransformOperation {
    op.id("listFiles")
        .tag("files")
        .summary("List files")
        .description(
            "Returns metadata for every file the actor owns, including redacted \
             outputs (recognisable by `lineage`). Bytes are not loaded; fetch via \
             `GET /files/{id}/content` when needed.",
        )
}

#[tracing::instrument(target = TARGET, skip_all, fields(%id, %actor_id))]
async fn delete_file(
    State(engine): State<Engine>,
    ActorId(actor_id): ActorId,
    Path(FilePath { id }): Path<FilePath>,
) -> Result<StatusCode> {
    engine.registry().delete_file(actor_id, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

fn delete_file_docs(op: TransformOperation) -> TransformOperation {
    op.id("deleteFile")
        .tag("files")
        .summary("Delete a file")
        .description(
            "Removes the file's bytes + metadata. Does not cascade — runs that \
             referenced the file remain (their `inputFileId` will resolve to a \
             missing file on subsequent apply).",
        )
}

#[tracing::instrument(target = TARGET, skip_all, fields(%actor_id))]
async fn delete_all_files(
    State(engine): State<Engine>,
    ActorId(actor_id): ActorId,
) -> Result<StatusCode> {
    let removed = engine.registry().delete_all_files(actor_id).await?;
    tracing::info!(target: TARGET, removed, "all files deleted");
    Ok(StatusCode::NO_CONTENT)
}

fn delete_all_files_docs(op: TransformOperation) -> TransformOperation {
    op.id("deleteAllFiles")
        .tag("files")
        .summary("Delete every file for the actor")
}

pub fn routes_v1() -> ApiRouter<ServiceState> {
    let read = ApiRouter::new()
        .api_route("/files", get_with(list_files, list_files_docs))
        .api_route("/files/{id}", get_with(get_file, get_file_docs))
        .api_route(
            "/files/{id}/content",
            get_with(get_file_content, get_file_content_docs),
        )
        .with_timeout(DEFAULT_READ_TIMEOUT);

    let write = ApiRouter::new()
        .api_route(
            "/files",
            post_with(upload_file, upload_file_docs)
                .delete_with(delete_all_files, delete_all_files_docs),
        )
        .api_route("/files/{id}", delete_with(delete_file, delete_file_docs))
        .with_timeout(DEFAULT_WRITE_TIMEOUT);

    read.merge(write)
}

/// Parse the `filename="..."` value out of a `Content-Disposition`
/// header. Accepts quoted and unquoted forms.
fn parse_filename(headers: &HeaderMap) -> Option<String> {
    let raw = headers
        .get(header::CONTENT_DISPOSITION)
        .and_then(|h| h.to_str().ok())?;
    for part in raw.split(';') {
        let part = part.trim();
        if let Some(rest) = part.strip_prefix("filename=") {
            let unquoted = rest.trim_matches('"');
            return Some(unquoted.to_owned());
        }
    }
    None
}

/// Extract the lowercase extension from a filename. `report.pdf`
/// → `Some("pdf")`; `noext` → `None`.
fn filename_extension(name: &str) -> Option<&str> {
    let idx = name.rfind('.')?;
    if idx == 0 || idx + 1 >= name.len() {
        return None;
    }
    Some(&name[idx + 1..])
}
