//! [`ExtractorOutput`]: paired return shape for an
//! [`Extractor<M>`].
//!
//! Bundles the extractor's backend-shaped `value` with the modality-
//! keyed `extraction` provenance the document stamps at extraction
//! time. Lifting provenance into the return type means consumers
//! capture both pieces from a single call rather than reaching for a
//! separate `extraction()` accessor on the extractor.
//!
//! [`Extractor<M>`]: crate::extraction::Extractor

use crate::modality::Modality;

/// Combined extractor return shape: a per-call `value` and the
/// modality-keyed `extraction` provenance produced alongside it.
#[derive(Debug, Clone)]
pub struct ExtractorOutput<M: Modality, T> {
    /// Backend-shaped extractor output (e.g. `Vec<OcrOutput>` for an
    /// image extractor, `Transcription` for an audio extractor).
    pub value: T,
    /// Per-modality provenance value the document's metadata records
    /// at extraction time. Same value for every call from a given
    /// extractor instance — folded into the output so consumers
    /// don't keep a separate accessor.
    pub extraction: M::Extraction,
}

impl<M: Modality, T> ExtractorOutput<M, T> {
    /// Construct an output from its two parts.
    pub fn new(value: T, extraction: M::Extraction) -> Self {
        Self { value, extraction }
    }
}
