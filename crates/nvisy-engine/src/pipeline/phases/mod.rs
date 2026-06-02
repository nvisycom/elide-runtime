//! Pipeline phase orchestrators.
//!
//! Each phase here is the document-walking glue around its
//! respective subsystem types. The subsystem types themselves
//! ([`RecognizerRegistry`], [`ExtractionEngine`], etc.) stay free
//! of [`Document`] knowledge so they can be exercised standalone
//! (tests, custom drivers).
//!
//! Phases are concrete structs (no `Phase<M>` trait); ordering is a
//! type-level fact set by the pipeline orchestrator.
//!
//! [`Document`]: nvisy_ontology::document::Document
//! [`RecognizerRegistry`]: crate::detection::RecognizerRegistry
//! [`ExtractionEngine`]: crate::extraction::ExtractionEngine

mod deduplication;
mod detection;
mod extraction;
mod redaction;
mod validation;

pub use self::deduplication::DeduplicationPhase;
pub use self::detection::DetectionPhase;
pub use self::extraction::ExtractionPhase;
pub use self::redaction::RedactionPhase;
pub use self::validation::ValidationPhase;
