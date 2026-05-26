//! Image modality.

use derive_builder::Builder;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{Mergeable, Modality, Overlap};
use crate::primitive::{BoundingBox, Confidence, LanguageDetection, Polygon};

/// A region within image content.
#[derive(Debug, Clone, PartialEq, Builder)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[builder(
    name = "ImageBuilder",
    pattern = "owned",
    setter(into, strip_option, prefix = "with")
)]
#[serde(rename_all = "camelCase")]
pub struct Image {
    /// Axis-aligned bounding box of the region.
    pub bounding_box: BoundingBox,
    /// Polygon vertices for the region when the source produced a
    /// rotated or quadrilateral shape (OCR engines that emit 4-point
    /// polygons populate this; axis-aligned-only sources leave it
    /// unset).
    #[builder(default, setter(into = false))]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub polygon: Option<Polygon>,
    /// Links this region to a specific image document.
    #[builder(default, setter(into = false))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_id: Option<Uuid>,
    /// 1-based page number (for multi-page documents like PDFs).
    #[builder(default, setter(into = false))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_number: Option<u32>,
}

impl Image {
    /// Create an [`Image`] from the bounding box alone, with every
    /// optional field unset. Use [`builder`] when polygon, `image_id`,
    /// or `page_number` need to be set.
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

    /// Create a new [`ImageBuilder`].
    pub fn builder() -> ImageBuilder {
        ImageBuilder::default()
    }

    /// Area of the bounding box in pixels (`width * height`).
    pub fn area(&self) -> f64 {
        self.bounding_box.width * self.bounding_box.height
    }
}

impl Modality for Image {
    type BlockKind = ImageBlockKind;
    type Artefact = ImageArtefact;
    type Metadata = ImageMetadata;
}

/// Classification of a [`Block<Image>`].
///
/// [`Block<Image>`]: crate::document::Block
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ImageBlockKind {
    /// A region of recognized text (paragraph, line, OCR text block).
    #[default]
    Text,
    /// A heading.
    Heading,
    /// A tabular region recognized in the image.
    Table,
    /// A figure or illustration containing recognized text.
    Figure,
    /// Anything else (margin notes, captions, headers/footers, …).
    Other,
}

/// A non-textual element detected in a [`Block<Image>`].
///
/// [`Block<Image>`]: crate::document::Block
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ImageArtefact {
    /// What kind of artefact this is.
    pub kind: ImageArtefactKind,
    /// Recognition confidence for the artefact.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confidence: Option<Confidence>,
    /// Where in the image the artefact was detected.
    pub region: Image,
}

/// Kinds of [`ImageArtefact`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ImageArtefactKind {
    /// A figure, illustration or photograph.
    Figure,
    /// A separator (rule, line, divider).
    Separator,
    /// A background element (watermark, fill, decoration).
    Background,
    /// A logo or brand mark.
    Logo,
    /// Anything else not covered above.
    Other,
}

/// Document-level metadata for [`Document<Image>`].
///
/// [`Document<Image>`]: crate::document::Document
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ImageMetadata {
    /// Languages detected (or asserted) for the document content.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[schemars(skip)]
    pub languages: Vec<LanguageDetection>,
    /// Page dimensions for multi-page sources (PDFs, multi-image
    /// uploads). Empty for single-page sources.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pages: Vec<PageDimensions>,
}

/// Dimensions of a single page in an image document.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PageDimensions {
    /// 1-based page number.
    pub number: u32,
    /// Page width in pixels.
    pub width: f64,
    /// Page height in pixels.
    pub height: f64,
}

impl Overlap for Image {
    fn overlaps(&self, other: &Self) -> bool {
        self.bounding_box.overlaps(&other.bounding_box)
    }
}

impl Mergeable for Image {
    /// Merge two image regions by unioning bounding boxes when their
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
