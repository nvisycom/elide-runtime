//! Image-modality entity location.

use derive_builder::Builder;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{Mergeable, Overlap};
use crate::primitive::{BoundingBox, Polygon};

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
    /// Axis-aligned bounding box of the entity in the image.
    pub bounding_box: BoundingBox,
    /// Polygon vertices for the region when the source produced a
    /// rotated or quadrilateral shape (OCR engines that emit
    /// 4-point polygons populate this; axis-aligned-only sources
    /// leave it unset).
    #[builder(default, setter(into = false))]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub polygon: Option<Polygon>,
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
    /// Create an [`ImageLocation`] from the bounding box alone, with
    /// every optional field unset. Use [`builder`] when polygon,
    /// `image_id`, or `page_number` need to be set.
    ///
    /// [`builder`]: Self::builder
    pub fn new(bounding_box: BoundingBox) -> Self {
        Self {
            bounding_box,
            polygon: None,
            image_id: None,
            page_number: None,
        }
    }

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
    ///
    /// The polygon is dropped on merge — the convex hull of two
    /// rotated quads is not well defined as another quad, and the
    /// unioned bbox already captures the merged region.
    fn try_merge(self, other: Self) -> Option<Self> {
        if self.image_id != other.image_id || self.page_number != other.page_number {
            return None;
        }
        Some(Self {
            bounding_box: self.bounding_box.union(&other.bounding_box),
            polygon: None,
            image_id: self.image_id,
            page_number: self.page_number,
        })
    }
}
