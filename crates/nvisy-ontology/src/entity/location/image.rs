//! Image-modality entity location.

use derive_builder::Builder;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{Mergeable, Overlap};
use crate::primitive::BoundingBox;

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

    /// Area of the bounding box in pixels (`width * height`).
    pub fn area(&self) -> f64 {
        self.bounding_box.width * self.bounding_box.height
    }
}

impl Overlap for ImageLocation {
    fn overlaps(&self, other: &Self) -> bool {
        self.bounding_box.overlaps(&other.bounding_box)
    }
}

impl Mergeable for ImageLocation {
    /// Merge two image locations by unioning bounding boxes when their
    /// `image_id` and `page_number` match. Different documents or
    /// different pages cannot merge.
    fn try_merge(self, other: Self) -> Option<Self> {
        if self.image_id != other.image_id || self.page_number != other.page_number {
            return None;
        }
        Some(Self {
            bounding_box: self.bounding_box.union(&other.bounding_box),
            image_id: self.image_id,
            page_number: self.page_number,
        })
    }
}
