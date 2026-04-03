//! Operations: the building blocks of the redaction pipeline.
//!
//! Each operation file corresponds to a [`GraphNodeKind`] variant and
//! implements the [`Operation`] trait, receiving and mutating a
//! [`DocumentEnvelope`] directly.
//!
//! [`GraphNodeKind`]: nvisy_ontology::workflow::GraphNodeKind

pub(crate) mod compression;
pub mod encryption;
mod entity_recognition;
pub mod envelope;
mod export_file;
mod fusion;
mod generate_context;
mod import_file;
#[allow(dead_code)]
mod load_context;
mod pattern_recognition;
pub(crate) mod redaction;
mod save_context;
mod speech;
mod validation;
mod vision;

pub(crate) use self::entity_recognition::EntityRecognitionOp;
pub use self::envelope::DocumentEnvelope;
pub(crate) use self::export_file::ExportFileOp;
pub(crate) use self::fusion::FusionOp;
pub(crate) use self::generate_context::GenerateContextOp;
pub(crate) use self::import_file::ImportFileOp;
pub(crate) use self::pattern_recognition::PatternRecognitionOp;
pub(crate) use self::redaction::RedactionOp;
pub(crate) use self::save_context::SaveContextOp;
pub(crate) use self::speech::AudialExtractionOp;
pub(crate) use self::validation::ValidationOp;
pub(crate) use self::vision::VisualExtractionOp;

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
