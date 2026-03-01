//! Multipart upload extractor.
//!
//! Provides the [`Upload`] struct as an axum `FromRequest` extractor that
//! consumes a `multipart/form-data` body and yields the uploaded file bytes,
//! optional filename, and optional content type.

use aide::OperationInput;
use axum::extract::{FromRequest, Multipart, Request};

use crate::handler::error::{Error, ErrorKind};

/// Parsed multipart upload payload.
///
/// Extracted from a `multipart/form-data` request containing a `file` field
/// and an optional `content_type` text field.
pub struct Upload {
    pub bytes: Vec<u8>,
    pub filename: Option<String>,
    pub content_type: Option<String>,
}

impl<S: Send + Sync> FromRequest<S> for Upload {
    type Rejection = Error<'static>;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        let mut multipart = Multipart::from_request(req, state)
            .await
            .map_err(|e| ErrorKind::BadRequest.with_message(format!("multipart error: {e}")))?;

        let mut file_bytes: Option<Vec<u8>> = None;
        let mut filename: Option<String> = None;
        let mut content_type: Option<String> = None;

        while let Some(field) = multipart
            .next_field()
            .await
            .map_err(|e| ErrorKind::BadRequest.with_message(format!("multipart error: {e}")))?
        {
            let Some(field_name) = field.name().map(str::to_owned) else {
                tracing::warn!("ignoring multipart field with no name");
                continue;
            };
            match field_name.as_str() {
                "file" => {
                    filename = field.file_name().map(String::from);
                    content_type = field.content_type().map(String::from);
                    file_bytes = Some(
                        field
                            .bytes()
                            .await
                            .map_err(|e| {
                                ErrorKind::BadRequest
                                    .with_message(format!("failed to read file field: {e}"))
                            })?
                            .to_vec(),
                    );
                }
                "content_type" => {
                    let value = field.text().await.map_err(|e| {
                        ErrorKind::BadRequest
                            .with_message(format!("failed to read content_type field: {e}"))
                    })?;
                    content_type = Some(value);
                }
                _ => {
                    tracing::debug!(field = field_name, "ignoring unknown multipart field");
                }
            }
        }

        let bytes = file_bytes.ok_or_else(|| {
            ErrorKind::BadRequest.with_message("missing required 'file' field")
        })?;

        Ok(Self {
            bytes,
            filename,
            content_type,
        })
    }
}

impl OperationInput for Upload {}
