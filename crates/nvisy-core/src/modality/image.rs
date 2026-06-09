//! [`Image`] modality marker, [`ImageLocation`] coordinate type,
//! [`ImageData`] per-call payload, and [`ImageExtraction`] provenance
//! enum.

use bytes::Bytes;
use hipstr::HipStr;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{Modality, Overlap};
use crate::entity::ModelProvenance;
use crate::primitive::{BoundingBox, Dimensions, Polygon};
use crate::redaction::ImageReplacement;

/// Image modality marker (zero-sized).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct Image;

impl Modality for Image {
    type Data = ImageData;
    type Extraction = ImageExtraction;
    type Location = ImageLocation;
    type Replacement = ImageReplacement;
}

/// A region within image content.
#[derive(Debug, Clone, PartialEq)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ImageLocation {
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

impl ImageLocation {
    /// Create an [`ImageLocation`] from the bounding box alone, with
    /// every optional field unset.
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

/// Per-call payload for [`Image`] recognizers and extractors.
///
/// The pixel dimensions are needed alongside the encoded bytes
/// because recognizers that emit normalised bounding boxes scale
/// them to pixel coordinates using `dims`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ImageData {
    /// Encoded image bytes.
    pub bytes: Bytes,
    /// Pixel dimensions of the encoded image.
    pub dims: Dimensions,
    /// Original filename, when known. Useful for diagnostics and for
    /// downstream consumers that want to infer the encoding from the
    /// extension.
    pub filename: Option<HipStr<'static>>,
}

impl ImageData {
    /// Construct with the encoded bytes and pixel dimensions; filename
    /// is initially unset.
    pub fn new(bytes: impl Into<Bytes>, dims: Dimensions) -> Self {
        Self {
            bytes: bytes.into(),
            dims,
            filename: None,
        }
    }

    /// Attach an original filename hint.
    pub fn with_filename(mut self, filename: impl Into<HipStr<'static>>) -> Self {
        self.filename = Some(filename.into());
        self
    }

    /// Extension derived from [`filename`], or `"png"` when no
    /// filename is set or the filename has no extension.
    ///
    /// [`filename`]: Self::filename
    pub fn extension(&self) -> &str {
        self.filename
            .as_deref()
            .and_then(|name| name.rsplit_once('.'))
            .map(|(_, ext)| ext)
            .unwrap_or("png")
    }
}

impl Overlap for ImageLocation {
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

/// How a [`Document<Image>`]'s content was produced.
///
/// Every image-modality document is the output of *some* recognition
/// pass over pixels; the variant names which pass ran. [`Pending`] is
/// the importer-time placeholder before any extractor has run; the
/// extractor stage replaces it with the concrete variant carrying the
/// backend's [`ModelProvenance`].
///
/// [`Document<Image>`]: # "carrier owned by nvisy-engine"
/// [`Pending`]: Self::Pending
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[non_exhaustive]
pub enum ImageExtraction {
    /// No extractor has run yet. Importer stamps this; the extractor
    /// stage replaces it once an OCR / scene-text / object-detection /
    /// layout backend produces blocks.
    Pending,
    /// Optical character recognition: raster text (printed or
    /// handwritten) converted into machine-readable characters.
    Ocr(ModelProvenance),
    /// Scene text detection: text embedded in natural images (signs,
    /// screens, whiteboards) localised prior to OCR.
    SceneText(ModelProvenance),
    /// Object detection: regions of interest located and labelled
    /// within an image or video frame.
    ObjectDetection(ModelProvenance),
    /// Document layout analysis: structural regions (headers, footers,
    /// signature blocks, form fields) identified by spatial
    /// arrangement rather than content.
    LayoutAnalysis(ModelProvenance),
}
