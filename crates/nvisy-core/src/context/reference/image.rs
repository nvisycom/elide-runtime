//! Image reference data for object matching.

use elide_core::primitive::BoundingBox;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Reference image for object/scene matching.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ImageData {
    /// Id of the file holding the reference image.
    pub image_source: Uuid,
    /// Optional sub-region within the image.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub region: Option<BoundingBox>,
}
