//! Image modality.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{ImageExtraction, Mergeable, Modality, ModalityBlock, Overlap};
use crate::policy::ImageStrategy;
use crate::primitive::{BoundingBox, LanguageDetection, Polygon};

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

impl Modality for Image {
    type Block = ImageBlock;
    type Metadata = ImageMetadata;
    type MethodTag = crate::policy::ImageMethodTag;
    /// Image audits record only which method ran; the substitution
    /// is a binary pixel transform whose parameters live on
    /// `ImageStrategy`.
    type Replacement = crate::policy::ImageMethodTag;
    type Strategy = ImageStrategy;

    fn default_method_dominance() -> &'static [Self::MethodTag] {
        // Block destroys colour entirely; Pixelate leaks coarse
        // colour; Blur leaks low-frequency colour + edges. When in
        // doubt, redact harder.
        &[
            crate::policy::ImageMethodTag::Block,
            crate::policy::ImageMethodTag::Pixelate,
            crate::policy::ImageMethodTag::Blur,
        ]
    }
}

/// Per-modality block payload for [`Image`]. Text-bearing variants
/// carry recognized text; per-word source spans live on the wrapping
/// [`Block<Image>`]. Non-textual variants ([`Figure`], [`Separator`],
/// [`Background`], [`Logo`]) carry no text. Every variant carries the
/// bounding `region` since image blocks are always spatially located.
///
/// [`Figure`]: Self::Figure
/// [`Separator`]: Self::Separator
/// [`Background`]: Self::Background
/// [`Logo`]: Self::Logo
/// [`Block<Image>`]: crate::document::Block
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[non_exhaustive]
pub enum ImageBlock {
    /// A region of recognized text (paragraph, line, OCR text block).
    Text { region: Image, text: String },
    /// A heading.
    Heading { region: Image, text: String },
    /// A tabular region recognized in the image.
    Table { region: Image, text: String },
    /// A figure, illustration or photograph.
    Figure { region: Image },
    /// A separator (rule, line, divider).
    Separator { region: Image },
    /// A background element (watermark, fill, decoration).
    Background { region: Image },
    /// A logo or brand mark.
    Logo { region: Image },
}

impl ImageBlock {
    /// The image region this block occupies.
    pub fn region(&self) -> &Image {
        match self {
            Self::Text { region, .. }
            | Self::Heading { region, .. }
            | Self::Table { region, .. }
            | Self::Figure { region }
            | Self::Separator { region }
            | Self::Background { region }
            | Self::Logo { region } => region,
        }
    }

    /// Recognized text for text-bearing kinds, `None` for non-textual.
    pub fn text(&self) -> Option<&str> {
        match self {
            Self::Text { text, .. } | Self::Heading { text, .. } | Self::Table { text, .. } => {
                Some(text)
            }
            Self::Figure { .. }
            | Self::Separator { .. }
            | Self::Background { .. }
            | Self::Logo { .. } => None,
        }
    }
}

impl ModalityBlock for ImageBlock {
    fn scan_text(&self) -> Option<&str> {
        self.text()
    }
}

/// Document-level metadata for [`Document<Image>`].
///
/// [`Document<Image>`]: crate::document::Document
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ImageMetadata {
    /// How this document's image content was processed (OCR, scene
    /// text, object detection, layout analysis).
    pub extraction: ImageExtraction,
    /// Languages detected (or asserted) for the document content.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[schemars(skip)]
    pub languages: Vec<LanguageDetection>,
    /// Page dimensions for multi-page sources (PDFs, multi-image
    /// uploads). Empty for single-page sources.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pages: Vec<PageDimensions>,
}

impl From<ImageExtraction> for ImageMetadata {
    /// Build [`ImageMetadata`] carrying only the importer-known
    /// extraction tag. Languages and page dimensions start empty;
    /// downstream stages fill them in.
    fn from(extraction: ImageExtraction) -> Self {
        Self {
            extraction,
            languages: Vec::new(),
            pages: Vec::new(),
        }
    }
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

impl Mergeable for Image {
    /// Merge two image regions by unioning bounding boxes when their
    /// `image_id` and `page_number` match. Different documents or
    /// different pages cannot merge.
    ///
    /// The polygon is dropped on merge — the convex hull of two
    /// rotated quads is not well defined as another quad, and the
    /// unioned bbox already captures the merged region.
    fn try_merge(self, other: Self) -> Result<Self, (Self, Self)> {
        if self.image_id != other.image_id || self.page_number != other.page_number {
            return Err((self, other));
        }
        Ok(Self {
            bounding_box: self.bounding_box.union(&other.bounding_box),
            polygon: None,
            image_id: self.image_id,
            page_number: self.page_number,
        })
    }
}
