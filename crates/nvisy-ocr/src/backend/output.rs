//! OCR output types.

use serde::Serialize;

use nvisy_core::math::{BoundingBox, Polygon};
use nvisy_core::path::ContentSource;
use nvisy_ontology::location::TextLevel;

/// A single text region detected by an OCR backend.
#[derive(Debug, Clone, Serialize)]
pub struct ImageRegion {
    /// Extracted text content.
    pub text: String,
    /// Confidence score (0.0..=1.0), if the backend provides one.
    pub confidence: Option<f64>,
    /// Axis-aligned bounding box in pixel coordinates.
    pub bbox: BoundingBox,
    /// Polygon vertices for rotated or skewed text regions.
    pub polygon: Option<Polygon>,
    /// Hierarchical level of this text region: word, line, block, etc.
    pub level: Option<TextLevel>,
}

impl ImageRegion {
    /// Returns `true` if the extracted text is empty.
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    /// Length of the extracted text in bytes.
    pub fn text_len(&self) -> usize {
        self.text.len()
    }

    /// Area of the bounding box: width × height.
    pub fn area(&self) -> f64 {
        self.bbox.width * self.bbox.height
    }

    /// Returns `true` if the confidence meets or exceeds the given threshold.
    pub fn meets_threshold(&self, threshold: f64) -> bool {
        self.confidence.unwrap_or(0.0) >= threshold
    }
}

/// Output from an OCR run on a single image.
///
/// Groups detected [`ImageRegion`]s together with a [`ContentSource`]
/// derived from the input image for provenance tracking.
#[derive(Debug, Clone)]
pub struct ImageOutput {
    /// Provenance: derived from the input's [`ContentSource`].
    pub source: ContentSource,
    /// Text regions detected in the image.
    pub regions: Vec<ImageRegion>,
}

impl ImageOutput {
    /// Create an empty output with the given source.
    pub fn new(source: ContentSource) -> Self {
        Self {
            source,
            regions: Vec::new(),
        }
    }

    /// Insert a region into this output.
    pub fn insert(&mut self, region: ImageRegion) {
        self.regions.push(region);
    }

    /// Number of detected regions.
    pub fn len(&self) -> usize {
        self.regions.len()
    }

    /// Returns `true` if no regions were detected.
    pub fn is_empty(&self) -> bool {
        self.regions.is_empty()
    }

    /// Iterator over the detected regions.
    pub fn iter(&self) -> std::slice::Iter<'_, ImageRegion> {
        self.regions.iter()
    }

    /// Filter regions that meet the given confidence threshold.
    pub fn above_threshold(&self, threshold: f64) -> Vec<&ImageRegion> {
        self.regions
            .iter()
            .filter(|r| r.meets_threshold(threshold))
            .collect()
    }
}

impl<'a> IntoIterator for &'a ImageOutput {
    type Item = &'a ImageRegion;
    type IntoIter = std::slice::Iter<'a, ImageRegion>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl IntoIterator for ImageOutput {
    type Item = ImageRegion;
    type IntoIter = std::vec::IntoIter<ImageRegion>;

    fn into_iter(self) -> Self::IntoIter {
        self.regions.into_iter()
    }
}
