//! Operations: units of work in the redaction pipeline.
//!
//! Each operation implements the [`Operation`] trait and belongs to one of
//! three provenance categories:
//!
//! | Category       | Module        | Examples                          |
//! |---------------|---------------|-----------------------------------|
//! | Inference     | [`inference`] | OCR, NER, transcription, CV, …    |
//! | Processing    | [`processing`]| Redaction, pattern match, …       |
//! | Lifecycle     | [`lifecycle`] | Ingest, publish, encryption, …    |

mod context;
pub mod inference;
pub mod lifecycle;
pub mod processing;

use std::future::Future;

pub use context::{OperationContext, ParallelContext, SequentialContext, SharedContext};
use nvisy_core::Result;

/// A single unit of work in the redaction pipeline.
///
/// Operations are stateless and composable. The engine calls [`Operation::call`]
/// with an input value and the operation produces a typed output or an error.
///
/// Both `Input` and `Output` must implement [`OperationContext`], encoding the
/// processing strategy (e.g. [`ParallelContext<Vec<Entity>>`] or
/// [`SequentialContext<Vec<Span>>`]) directly in the type.
pub trait Operation {
    /// Data consumed by this operation: wraps the payload in a context marker.
    type Input: OperationContext;
    /// Data produced by this operation: wraps the payload in a context marker.
    type Output: OperationContext;

    /// Execute the operation.
    fn call(&self, input: Self::Input) -> impl Future<Output = Result<Self::Output>> + Send;
}
