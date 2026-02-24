use schemars::JsonSchema;
use serde::Serialize;
use uuid::Uuid;

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
