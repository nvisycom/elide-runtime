//! Operations: the building blocks of the redaction pipeline.
//!
//! Each operation file corresponds to a [`GraphNodeKind`] variant and
//! implements the [`Operation`] trait with typed inputs and outputs.
//!
//! | File                      | Graph node                | Purpose                              |
//! |---------------------------|---------------------------|--------------------------------------|
//! | [`import_file`]           | `ImportFile`              | Load content from registry, decode   |
//! | [`export_file`]           | `ExportFile`              | Collect results for delivery         |
//! | [`vision`]                | `VisualExtraction`        | OCR + verification + CV              |
//! | [`speech`]                | `AudialExtraction`        | Speech-to-text transcription         |
//! | [`entity_recognition`]    | `NamedEntityRecognition`  | NER via language model               |
//! | [`pattern_recognition`]   | `PatternRecognition`      | Regex + dictionary detection         |
//! | [`fusion`]                | `Fusion`                  | Deduplication + confidence merge     |
//! | [`redaction`]             | `Redaction`               | Policy evaluation + content redaction|
//! | [`validation`]            | `Validation`              | Post-redaction leak detection        |
//! | [`load_context`]          | `LoadContext`             | Load contexts from registry          |
//! | [`save_context`]          | `SaveContext`             | Persist contexts to registry         |
//! | [`generate_context`]      | `GenerateContext`         | Generate contexts from results       |
//!
//! [`GraphNodeKind`]: crate::graph::GraphNodeKind

pub(crate) mod compression;
mod context;
pub(crate) mod encryption;
mod entity_recognition;
pub mod envelope;
mod export_file;
mod fusion;
mod generate_context;
mod import_file;
mod load_context;
mod pattern_recognition;
mod redaction;
mod save_context;
mod speech;
mod validation;
mod vision;

use std::future::Future;

use nvisy_core::Result;

pub use self::context::{OperationContext, ParallelContext, SequentialContext, SharedContext};
pub(crate) use self::entity_recognition::EntityRecognition;
pub use self::envelope::DocumentEnvelope;
pub(crate) use self::fusion::Fusion;
pub(crate) use self::generate_context::GenerateContext;
pub(crate) use self::import_file::ImportFile;
pub(crate) use self::load_context::LoadContext;
pub(crate) use self::pattern_recognition::PatternRecognition;
pub(crate) use self::redaction::Redaction;
pub(crate) use self::save_context::SaveContext;
pub(crate) use self::speech::AudialExtraction;
pub(crate) use self::validation::Validation;
pub(crate) use self::vision::VisualExtraction;

/// A single unit of work in the redaction pipeline.
///
/// Operations are stateless and composable. The engine calls [`Operation::call`]
/// with an input value and the operation produces a typed output or an error.
///
/// Both `Input` and `Output` must implement [`OperationContext`], encoding the
/// processing strategy (e.g. [`ParallelContext<Entities>`] or
/// [`SequentialContext<Vec<Span>>`]) directly in the type.
pub trait Operation {
    /// Data consumed by this operation: wraps the payload in a context marker.
    type Input: OperationContext;
    /// Data produced by this operation: wraps the payload in a context marker.
    type Output: OperationContext;

    /// Execute the operation.
    fn call(&self, input: Self::Input) -> impl Future<Output = Result<Self::Output>> + Send;
}
