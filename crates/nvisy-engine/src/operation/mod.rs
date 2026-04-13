//! Operations: the building blocks of the redaction pipeline.
//!
//! Each operation implements the [`Operation`] trait, receiving and
//! mutating a [`DocumentEnvelope`] directly.
//!
//! # Submodules
//!
//! - [`extraction`]: visual (OCR), audial (STT), and text extraction.
//! - [`detection`]: NER and pattern-based entity detection.
//! - [`deduplication`]: entity deduplication and confidence scoring.
//! - [`redaction`]: policy evaluation and content redaction.

mod deduplication;
mod detection;
pub mod envelope;
mod export_file;
mod extraction;
mod generate_context;
mod import_file;
pub(crate) mod redaction;
mod validation;

pub(crate) use self::deduplication::DeduplicationOp;
pub(crate) use self::detection::{EntityRecognitionOp, PatternRecognitionOp};
pub use self::envelope::{Document, DocumentEnvelope};
pub(crate) use self::export_file::ExportFileOp;
pub(crate) use self::extraction::ExtractionOp;
pub(crate) use self::generate_context::GenerateContextOp;
pub(crate) use self::import_file::ImportFileOp;
pub(crate) use self::redaction::RedactionOp;
pub(crate) use self::validation::ValidationOp;

/// A single unit of work in the redaction pipeline.
///
/// Operations receive a mutable reference to the [`DocumentEnvelope`]
/// and read/write its fields directly. Run-wide shared state (policies,
/// registry, key provider) is accessible via `envelope.shared`.
pub trait Operation {
    /// Execute the operation, mutating the envelope in place.
    fn execute(
        &self,
        envelope: &mut DocumentEnvelope,
    ) -> impl Future<Output = nvisy_core::Result<()>> + Send;
}
