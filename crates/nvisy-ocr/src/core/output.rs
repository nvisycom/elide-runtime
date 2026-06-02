//! [`OcrOutput`]: backend-shaped recognition output.
//!
//! The OCR `Backend` trait returns these instead of
//! `nvisy_document::document::Block<Image>` to keep `nvisy-ocr` free
//! of any `nvisy-document` dependency (the orchestrator dep
//! direction is core ← toolkit ← document, with detection backends
//! (this crate) ← document — and the trait outputs must follow that
//! direction).
//!
//! The document-side OCR extraction phase wraps each `OcrOutput` into
//! a [`Block<Image>`] (filling in the span list and confidence). The
//! lossless 1:1 mapping is documented at the phase site.

use nvisy_core::modality::Image;
use nvisy_core::primitive::Confidence;

/// Backend-shaped recognition output. One per recognized text region.
///
/// Mirrors the data a document-side `Block<Image>` carries minus the
/// wrapping container: the per-modality block payload (text + region)
/// plus the per-word spans the recognizer emitted.
#[derive(Debug, Clone)]
pub struct OcrOutput {
    /// The image-modality block payload — recognized text + bounding region.
    pub kind: OcrBlockKind,
    /// Per-word source spans in the recognized text.
    pub spans: Vec<OcrSpan>,
    /// Block-level confidence.
    pub confidence: Confidence,
}

/// Subset of `ImageBlock` variants OCR backends emit.
///
/// OCR doesn't produce `Figure`/`Separator`/`Background`/`Logo` —
/// those are layout-analysis outputs. Only text-bearing variants land
/// here.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum OcrBlockKind {
    /// A region of recognized text.
    Text { region: Image, text: String },
    /// A heading.
    Heading { region: Image, text: String },
    /// A tabular region recognized in the image.
    Table { region: Image, text: String },
}

/// Per-word span emitted by an OCR backend.
#[derive(Debug, Clone)]
pub struct OcrSpan {
    /// Byte offset where the word starts in the block text.
    pub text_start: usize,
    /// Byte offset where the word ends in the block text.
    pub text_end: usize,
    /// Modality-typed source location (per-word bounding region).
    pub source: Image,
    /// Per-word confidence.
    pub confidence: Confidence,
}
