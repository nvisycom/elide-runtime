//! Image-modality entity location.

use derive_builder::Builder;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::Overlap;
use crate::math::BoundingBox;

/// Location of an entity within an image.
#[derive(Debug, Clone, PartialEq, Builder)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[builder(
    name = "ImageLocationBuilder",
    pattern = "owned",
    setter(into, strip_option, prefix = "with")
)]
#[serde(rename_all = "camelCase")]
pub struct ImageLocation {
    /// Bounding box of the entity in the image.
    pub bounding_box: BoundingBox,
    /// OCR-extracted text value at this location, if available.
    #[builder(default, setter(into = false))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    /// Links this entity to a specific image document.
    #[builder(default, setter(into = false))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_id: Option<Uuid>,
    /// 1-based page number (for multi-page documents like PDFs).
    #[builder(default, setter(into = false))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_number: Option<u32>,
}

impl ImageLocation {
    /// Create a new [`ImageLocationBuilder`].
    pub fn builder() -> ImageLocationBuilder {
        ImageLocationBuilder::default()
    }
}

impl Overlap for ImageLocation {
    fn overlaps(&self, other: &Self) -> bool {
        self.bounding_box.overlaps(&other.bounding_box)
    }
}
