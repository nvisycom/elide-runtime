//! Detection: per-modality [`RecognizerRegistry`].
//!
//! This module is intentionally narrow: it owns the registry of
//! recognizers (a pair of `Vec<Arc<dyn Recognizer<M>>>`), nothing
//! more. The registry takes a [`Context`] and runs recognizers; it
//! has no knowledge of [`Document`], blocks, or pipeline phases.
//!
//! The Document-walking glue that drives the registry per-block /
//! per-image lives in [`DetectionPhase`] —
//! that's where block iteration, entity-kind filtering, and
//! span-to-location lifting happen.
//!
//! Pattern and NER are the active recognizer types. LLM and VLM
//! were removed pending a rework to implement
//! [`nvisy_core::Recognizer<M>`] directly; their implementations
//! lived under this module historically and can be reconstructed
//! from git history if/when they come back.
//!
//! [`Context`]: nvisy_core::Context
//! [`Document`]: nvisy_ontology::document::Document
//! [`DetectionPhase`]: crate::pipeline::DetectionPhase

mod registry;

pub use self::registry::RecognizerRegistry;
