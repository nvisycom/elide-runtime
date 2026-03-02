//! Image-modality reference data.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use nvisy_core::math::BoundingBox;
use nvisy_core::path::ContentSource;

/// Image reference (face, object, logo, document, etc.).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ImageData {
    /// Source pointer to the reference image.
    pub image_source: ContentSource,
    /// Optional sub-region within the image.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub region: Option<BoundingBox>,
}
