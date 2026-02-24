use aide::axum::IntoApiResponse;
use axum::extract::{Path, State};
use axum::Json;
use nvisy_core::io::{Content, ContentData};
use nvisy_core::{Error, ErrorKind};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::error::ServerError;
use crate::service::ServiceState;

/// Request body for `POST /api/v1/content`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct UploadRequest {
    /// Base64-encoded content bytes.
    pub content: String,
    /// Optional original filename.
    #[serde(default)]
    pub filename: Option<String>,
    /// Optional MIME type hint.
    #[serde(default)]
    pub content_type: Option<String>,
}

/// Response body for `POST /api/v1/content`.
#[derive(Debug, Serialize, JsonSchema)]
pub struct UploadResponse {
    /// Identifier assigned to the uploaded content.
    pub id: Uuid,
}

/// Response body for `GET /api/v1/content/{id}`.
#[derive(Debug, Serialize, JsonSchema)]
pub struct DownloadResponse {
    /// Identifier of the content.
    pub id: Uuid,
    /// Base64-encoded content bytes.
    pub content: String,
}

/// `POST /api/v1/content` — upload content for later processing.
pub async fn upload(
    State(state): State<ServiceState>,
    Json(req): Json<UploadRequest>,
) -> Result<impl IntoApiResponse, ServerError> {
    use base64::Engine as _;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(&req.content)
        .map_err(|e| Error::new(ErrorKind::InvalidInput, format!("invalid base64: {e}")))?;

    let mut content_data = ContentData::from(bytes);
    if let Some(ref mime) = req.content_type {
        content_data.mime = Some(mime.clone());
    }
    let content = Content::new(content_data);
    let handler = state.content_registry.register(content).await?;

    tracing::info!(
        id = %handler.content_source(),
        filename = req.filename.as_deref().unwrap_or("<none>"),
        "content uploaded",
    );

    Ok(Json(UploadResponse {
        id: handler.content_source().as_uuid(),
    }))
}

/// `GET /api/v1/content/{id}` — download previously uploaded content.
pub async fn download(
    State(_state): State<ServiceState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoApiResponse, ServerError> {
    Err::<Json<DownloadResponse>, _>(ServerError::from(Error::new(
        ErrorKind::Runtime,
        format!("content download not yet implemented (id: {id})"),
    )))
}
