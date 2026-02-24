use aide::axum::IntoApiResponse;
use axum::extract::{Multipart, Path, State};
use axum::Json;
use nvisy_core::io::{Content, ContentData};
use nvisy_core::{Error, ErrorKind};
use uuid::Uuid;

use super::response::{DownloadResponse, ServerError, UploadResponse};
use crate::service::ServiceState;

/// `POST /api/v1/content` — upload content as multipart form data.
///
/// Expects a multipart form with a `file` field containing the binary content.
/// An optional `content_type` text field may override the detected MIME type.
#[tracing::instrument(skip_all)]
pub async fn upload(
    State(state): State<ServiceState>,
    mut multipart: Multipart,
) -> Result<impl IntoApiResponse, ServerError> {
    let mut file_bytes: Option<Vec<u8>> = None;
    let mut filename: Option<String> = None;
    let mut content_type: Option<String> = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| Error::new(ErrorKind::InvalidInput, format!("multipart error: {e}")))?
    {
        let field_name = field.name().unwrap_or_default().to_string();
        match field_name.as_str() {
            "file" => {
                filename = field.file_name().map(String::from);
                content_type = field.content_type().map(String::from);
                file_bytes = Some(
                    field
                        .bytes()
                        .await
                        .map_err(|e| {
                            Error::new(
                                ErrorKind::InvalidInput,
                                format!("failed to read file field: {e}"),
                            )
                        })?
                        .to_vec(),
                );
            }
            "content_type" => {
                let value = field.text().await.map_err(|e| {
                    Error::new(
                        ErrorKind::InvalidInput,
                        format!("failed to read content_type field: {e}"),
                    )
                })?;
                content_type = Some(value);
            }
            _ => {
                tracing::debug!(field = field_name, "ignoring unknown multipart field");
            }
        }
    }

    let bytes = file_bytes
        .ok_or_else(|| Error::new(ErrorKind::InvalidInput, "missing required 'file' field"))?;

    let size = bytes.len();
    let mut content_data = ContentData::from(bytes);
    if let Some(mime) = content_type {
        content_data.mime = Some(mime);
    }
    let content = Content::new(content_data);
    let handler = state.content_registry.register(content).await?;
    let id = handler.content_source().as_uuid();

    tracing::info!(
        %id,
        size,
        filename = filename.as_deref().unwrap_or("<none>"),
        "content uploaded",
    );

    Ok(Json(UploadResponse { id }))
}

/// `GET /api/v1/content/{id}` — download previously uploaded content.
#[tracing::instrument(skip_all, fields(%id))]
pub async fn download(
    State(_state): State<ServiceState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoApiResponse, ServerError> {
    Err::<Json<DownloadResponse>, _>(ServerError::from(Error::new(
        ErrorKind::Runtime,
        format!("content download not yet implemented (id: {id})"),
    )))
}
