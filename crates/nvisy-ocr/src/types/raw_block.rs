//! [`RawOcrBlock`]: pre-normalization OCR backend output.
//!
//! Emitted by an [`OcrBackend`]: one block per recognized text
//! region, with the per-modality kind, the per-word [`OcrSpan`]s,
//! and a block-level confidence. Consumers wrap each block into the
//! per-document block shape they need.
//!
//! [`OcrBackend`]: crate::backend::OcrBackend

use nvisy_core::modality::ImageLocation;
use nvisy_core::primitive::Confidence;

/// One raw OCR block predicted by a backend.
///
/// Pre-normalization: spans + confidence come straight from the
/// model; consumers translate the variant into their own per-block
/// shape and decide how to use the confidence.
#[derive(Debug, Clone)]
pub struct RawOcrBlock {
    /// The image-modality block payload — recognized text +
    /// bounding region.
    pub kind: OcrBlockKind,
    /// Per-word source spans in the recognized text.
    pub spans: Vec<OcrSpan>,
    /// Block-level confidence.
    pub confidence: Confidence,
}

/// Subset of `ImageBlock` variants OCR backends emit.
///
/// OCR doesn't produce `Figure`/`Separator`/`Background`/`Logo` —
/// those are layout-analysis outputs. Only text-bearing variants
/// land here.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum OcrBlockKind {
    /// A region of recognized text.
    Text {
        /// Bounding box of the recognized region in image coordinates.
        region: ImageLocation,
        /// Recognized text content.
        text: String,
    },
    /// A heading.
    Heading {
        /// Bounding box of the recognized region in image coordinates.
        region: ImageLocation,
        /// Recognized text content.
        text: String,
    },
    /// A tabular region recognized in the image.
    Table {
        /// Bounding box of the recognized region in image coordinates.
        region: ImageLocation,
        /// Recognized text content (cells flattened in reading order).
        text: String,
    },
}

/// Per-word span emitted by an OCR backend.
#[derive(Debug, Clone)]
pub struct OcrSpan {
    /// Byte offset where the word starts in the block text.
    pub text_start: usize,
    /// Byte offset where the word ends in the block text.
    pub text_end: usize,
    /// Modality-typed source location (per-word bounding region).
    pub source: ImageLocation,
    /// Per-word confidence.
    pub confidence: Confidence,
}
