//! Operations: the building blocks of the redaction pipeline.
//!
//! Each operation type exposes an inherent `execute(&self, envelope:
//! &mut DocumentEnvelope) -> Result<()>` method that the orchestrator
//! calls directly. No shared trait abstraction — every operation has
//! its own concrete type and dispatch site.

mod deduplication;
mod envelope;
mod export_file;
mod generate_context;
mod import_file;
mod validate;
mod validation_workflow;

pub(crate) use self::deduplication::Deduplicator;
pub use self::deduplication::workflow::{
    CalibrationMap, ConflictResolution, Deduplication, DeduplicationStrategy, GroupingCriteria,
};
pub(crate) use self::envelope::SharedData;
pub use self::envelope::{Document, DocumentEnvelope};
pub(crate) use self::export_file::Exporter;
pub(crate) use self::generate_context::ContextGenerator;
pub(crate) use self::import_file::Importer;
pub(crate) use self::validate::Validator;
pub use self::validation_workflow::Validation;
