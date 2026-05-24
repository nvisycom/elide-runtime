//! Extraction, recognition, and refinement method classification.
//!
//! These types form the provenance record for every detected entity,
//! documenting how content was extracted from its source modality,
//! how sensitive data was identified, and what post-detection
//! refinements were applied.

mod extraction;
mod provenance;
mod recognition;
mod refinement;

pub use self::extraction::ExtractionMethod;
pub use self::provenance::{
    AnnotationProvenance, CrossReferenceProvenance, ModelKind, ModelProvenance, PatternKind,
    PatternProvenance,
};
pub use self::recognition::{RecognitionMethod, RecognitionMethodKind};
pub use self::refinement::RefinementMethod;
