//! Operations: the building blocks of the redaction pipeline.
//!
//! Each operation implements the [`Operation`] trait, receiving and
//! mutating a [`DocumentEnvelope`] directly.
//!
//! # Submodules
//!
//! - `extraction`: visual (OCR), audial (STT), and text extraction.
//! - `detection`: NER and pattern-based entity detection.
//! - `deduplication`: entity deduplication and confidence scoring.
//! - `redaction`: policy evaluation and content redaction.

mod deduplication;
mod detection;
mod envelope;
mod export_file;
mod generate_context;
mod import_file;
mod validate;

use nvisy_core::Result;

pub(crate) use self::deduplication::Deduplication;
pub(crate) use self::detection::Detection;
pub(crate) use self::envelope::SharedData;
pub use self::envelope::{Document, DocumentEnvelope};
pub(crate) use self::export_file::ExportFile;
pub(crate) use self::generate_context::GenerateContext;
pub(crate) use self::import_file::ImportFile;
pub(crate) use self::validate::Validation;

/// A single unit of work in the redaction pipeline.
///
/// Operations receive a mutable reference to the [`DocumentEnvelope`]
/// and read/write its fields directly. Run-wide shared state (policies,
/// registry, key provider) is accessible via `envelope.shared`.
pub trait Operation {
    /// Execute the operation, mutating the envelope in place.
    fn execute(&self, envelope: &mut DocumentEnvelope) -> impl Future<Output = Result<()>> + Send;
}
