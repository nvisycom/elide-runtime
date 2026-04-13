//! Image reference data for object matching.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::entity::ContentSource;
use crate::primitive::BoundingBox;

/// Reference image for object/scene matching.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ImageData {
    /// Source pointer to the reference image.
    pub image_source: ContentSource,
    /// Optional sub-region within the image.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub region: Option<BoundingBox>,
    /// Image format hint (e.g. `"jpeg"`, `"png"`, `"webp"`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
}
