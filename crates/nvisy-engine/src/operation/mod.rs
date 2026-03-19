//! Operations: the building blocks of the redaction pipeline.
//!
//! Each operation file corresponds to a [`GraphNodeKind`] variant and
//! implements [`NodeHandler`] — a single async transform from
//! [`DocumentEnvelope`] to [`DocumentEnvelope`].
//!
//! | File                    | Graph node              | Purpose                              |
//! |-------------------------|-------------------------|--------------------------------------|
//! | [`import_file`]         | `Import`                | Load content from registry, decode   |
//! | [`export_file`]         | `Export`                 | Collect results for delivery         |
//! | [`vision`]              | `VisualExtraction`      | OCR + verification + CV              |
//! | [`speech`]              | `AudialExtraction`      | Speech-to-text transcription         |
//! | [`recognition`]         | `NamedEntityRecognition` / `PatternRecognition` | NER + regex + manual |
//! | [`fusion`]              | `Fusion`                | Deduplication + confidence merge     |
//! | [`redaction`]           | `Redaction`             | Policy evaluation + content redaction|
//! | [`validation`]          | `Validation`            | Post-redaction leak detection        |
//! | [`load_context`]        | `LoadContext`           | Load contexts from registry          |
//! | [`save_context`]        | `SaveContext`           | Persist contexts to registry         |
//! | [`generate_context`]    | `GenerateContext`       | Generate contexts from results       |
//!
//! [`GraphNodeKind`]: crate::graph::GraphNodeKind

mod context;
pub mod envelope;
mod export_file;
mod fusion;
mod generate_context;
mod import_file;
mod load_context;
mod recognition;
mod redaction;
mod save_context;
mod speech;
pub(crate) mod support;
mod validation;
mod vision;

use std::future::Future;

use nvisy_core::{Error, Result};

pub(crate) use self::speech::AudialExtraction;
pub use self::context::{OperationContext, ParallelContext, SequentialContext, SharedContext};
pub use self::envelope::DocumentEnvelope;
pub(crate) use self::fusion::Fusion;
pub(crate) use self::generate_context::GenerateContext;
pub(crate) use self::import_file::ImportFile;
pub(crate) use self::load_context::LoadContext;
pub(crate) use self::recognition::{EntityRecognition, PatternRecognition};
pub(crate) use self::redaction::Redaction;
pub(crate) use self::save_context::SaveContext;
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

/// Envelope-level transform for a pipeline node.
///
/// Each graph node kind has a corresponding struct that implements
/// this trait. The executor calls [`handle`](NodeHandler::handle)
/// for each envelope passing through the node.
#[async_trait::async_trait]
pub trait NodeHandler: Send + Sync {
    async fn handle(&self, envelope: DocumentEnvelope) -> Result<DocumentEnvelope, Error>;
}
