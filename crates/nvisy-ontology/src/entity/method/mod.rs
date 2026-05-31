//! Recognition and refinement method classification.
//!
//! These types form the per-entity provenance record: which technique
//! identified a sensitive data occurrence, and which post-detection
//! refinements ran. Document-level "how was the content produced"
//! (text-layer parse, OCR, transcription) is tracked separately on
//! the modality-specific extraction tag carried on
//! [`Document<M>::meta`] ([`TextExtraction`], [`ImageExtraction`],
//! [`AudioExtraction`], [`TabularExtraction`]) — that axis is a
//! property of the [`Document<M>`], not of the entities inside it.
//!
//! [`Document<M>`]: crate::document::Document
//! [`Document<M>::meta`]: crate::document::Document::meta
//! [`TextExtraction`]: crate::modality::TextExtraction
//! [`ImageExtraction`]: crate::modality::ImageExtraction
//! [`AudioExtraction`]: crate::modality::AudioExtraction
//! [`TabularExtraction`]: crate::modality::TabularExtraction

mod provenance;
mod recognition;
mod refinement;

pub use self::provenance::{
    AnnotationProvenance, CrossReferenceProvenance, ModelKind, ModelProvenance, PatternProvenance,
};
pub use self::recognition::{RecognitionMethod, RecognitionMethodKind};
pub use self::refinement::RefinementMethod;
