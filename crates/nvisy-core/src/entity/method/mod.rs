//! Per-entity provenance: who detected, who refined, and the
//! score-adjustment trail.
//!
//! Document-level "how was the content produced" (text-layer parse,
//! OCR, transcription) lives separately on the modality-specific
//! extraction tag carried on [`Document<M>::meta`]
//! ([`TextExtraction`], [`ImageExtraction`], [`AudioExtraction`],
//! [`TabularExtraction`]) — that axis is a property of the
//! [`Document<M>`], not of the entities inside it.
//!
//! [`Document<M>`]: https://docs.rs/nvisy-document/latest/nvisy_document/document/struct.Document.html
//! [`Document<M>::meta`]: https://docs.rs/nvisy-document/latest/nvisy_document/document/struct.Document.html#structfield.meta
//! [`TextExtraction`]: crate::modality::TextExtraction
//! [`ImageExtraction`]: crate::modality::ImageExtraction
//! [`AudioExtraction`]: crate::modality::AudioExtraction
//! [`TabularExtraction`]: crate::modality::TabularExtraction

mod provenance;
mod trail;

pub use self::provenance::{AnnotationProvenance, ModelProvenance, PatternProvenance};
pub use self::trail::{TrailProvenance, TrailStep, TrailStepKind};
