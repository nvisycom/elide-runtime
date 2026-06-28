//! Image reference data for object matching.

use elide_core::primitive::BoundingBox;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::ContentSource;

/// Reference image for object/scene matching.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ImageData {
    /// Source pointer to the reference image.
    pub image_source: ContentSource,
    /// Optional sub-region within the image.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub region: Option<BoundingBox>,
}
