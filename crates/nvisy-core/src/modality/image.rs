//! Image modality coordinate type.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{Modality, Overlap};
use crate::primitive::{BoundingBox, Polygon};

/// A region within image content.
#[derive(Debug, Clone, PartialEq)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Image {
    /// Axis-aligned bounding box of the region.
    pub bounding_box: BoundingBox,
    /// Polygon vertices for the region when the source produced a
    /// rotated or quadrilateral shape (OCR engines that emit 4-point
    /// polygons populate this; axis-aligned-only sources leave it
    /// unset).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub polygon: Option<Polygon>,
    /// Links this region to a specific image document.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_id: Option<Uuid>,
    /// 1-based page number (for multi-page documents like PDFs).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_number: Option<u32>,
}

impl Image {
    /// Create an [`Image`] from the bounding box alone, with every
    /// optional field unset.
    pub fn new(bounding_box: BoundingBox) -> Self {
        Self {
            bounding_box,
            polygon: None,
            image_id: None,
            page_number: None,
        }
    }

    /// Area of the bounding box in pixels (`width * height`).
    pub fn area(&self) -> f64 {
        self.bounding_box.width * self.bounding_box.height
    }
}

impl Modality for Image {}

impl Overlap for Image {
    /// Two image regions overlap only when they target the same
    /// image (matching `image_id`) on the same page (matching
    /// `page_number`) and their bounding boxes intersect. Without
    /// the image/page gates, two regions with the same bbox
    /// coordinates on different pages or different uploads would
    /// false-positive as overlapping.
    fn overlaps(&self, other: &Self) -> bool {
        self.image_id == other.image_id
            && self.page_number == other.page_number
            && self.bounding_box.overlaps(&other.bounding_box)
    }
}
