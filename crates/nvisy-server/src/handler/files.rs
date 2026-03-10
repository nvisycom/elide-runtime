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
use axum::extract::{Query, State};
use axum::http::StatusCode;
use nvisy_core::content::ContentMetadata;
use nvisy_core::content::{Content, ContentData};
use nvisy_registry::Registry;

use super::error::Result;
use super::request::{ActorQuery, ContentPath, NewFile};
use super::response::{File, FileId, FileList};
use super::utility::Base64;
use crate::extract::{Json, Path};
use crate::service::ServiceState;

const TARGET: &str = "nvisy_server::files";

/// `POST /api/v1/files`: upload a file as base64-encoded JSON.
#[tracing::instrument(
    target = "nvisy_server::files",
    skip_all,
    fields(%req.actor_id, filename = req.filename.as_deref()),
)]
async fn upload(
    State(registry): State<Registry>,
    Json(req): Json<NewFile>,
) -> Result<(StatusCode, Json<FileId>)> {
    let bytes = req.content.decode()?;
    let size = bytes.len();

    let mut content_data = ContentData::from(bytes);
    if let Some(ref mime) = req.content_type {
        content_data.mime = Some(mime.clone());
    }

    let mut metadata = match req.filename {
        Some(ref name) => ContentMetadata::with_path(name),
        None => ContentMetadata::new(),
    };
    if let Some(ref mime) = req.content_type {
        metadata.set_extra("content_type", serde_json::Value::String(mime.clone()));
    }

    let content = Content::with_metadata(content_data, metadata);
    let handle = registry.register_content(req.actor_id, content).await?;
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
#[tracing::instrument(
    target = "nvisy_server::files",
    skip_all,
    fields(%id, %actor_id),
)]
async fn download(
    State(registry): State<Registry>,
    Path(ContentPath { id }): Path<ContentPath>,
    Query(ActorQuery { actor_id }): Query<ActorQuery>,
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

fn download_docs(op: TransformOperation) -> TransformOperation {
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
async fn list(
    State(registry): State<Registry>,
    Query(ActorQuery { actor_id }): Query<ActorQuery>,
) -> Result<Json<FileList>> {
    let files = registry.list_content(actor_id).await?;
    tracing::debug!(target: TARGET, count = files.len(), "files listed");
    Ok(Json(FileList { files }))
}

fn list_docs(op: TransformOperation) -> TransformOperation {
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
async fn delete(
    State(registry): State<Registry>,
    Path(ContentPath { id }): Path<ContentPath>,
    Query(ActorQuery { actor_id }): Query<ActorQuery>,
) -> Result<StatusCode> {
    registry.unregister_content(actor_id, id).await?;
    tracing::info!(target: TARGET, "file deleted");
    Ok(StatusCode::NO_CONTENT)
}

fn delete_docs(op: TransformOperation) -> TransformOperation {
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
async fn delete_all(
    State(registry): State<Registry>,
    Query(ActorQuery { actor_id }): Query<ActorQuery>,
) -> Result<StatusCode> {
    let deleted = registry.unregister_all_content(actor_id).await?;
    tracing::info!(target: TARGET, deleted, "all files deleted");
    Ok(StatusCode::NO_CONTENT)
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
