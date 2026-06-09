//! Detection response types.

use nvisy_document::pipeline::DetectionEntry;
use schemars::JsonSchema;
use serde::Serialize;
use uuid::Uuid;

use super::page::Page;

/// Response body for `GET /detections`.
pub type DetectionList = Page<DetectionEntry>;

/// Response body for `POST /detections`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DetectionId {
    /// Identifier assigned to the submitted detection pass.
    pub id: Uuid,
}
