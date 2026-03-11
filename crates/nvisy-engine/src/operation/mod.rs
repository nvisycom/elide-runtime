//! Operations: composable units of work in the redaction pipeline.
//!
//! Each operation implements the [`Operation`] trait: a single async
//! function from typed input to typed output. Input and output are
//! wrapped in a context marker ([`ParallelContext`] or
//! [`SequentialContext`]) that tells the orchestrator how to invoke
//! the operation.
//!
//! Operations are grouped into three categories:
//!
//! | Category       | Module        | Purpose                                  |
//! |----------------|---------------|------------------------------------------|
//! | Inference      | [`inference`] | ML/AI model calls (OCR, NER, CV, …)     |
//! | Processing     | [`processing`]| Deterministic transforms (redact, match) |
//! | Lifecycle      | [`lifecycle`] | Content I/O (import, export, encrypt)    |

mod context;
pub mod inference;
pub mod lifecycle;
pub mod processing;
pub mod utility;

use std::future::Future;

pub use context::{
    DocumentEnvelope, OperationContext, ParallelContext, SequentialContext, SharedContext,
};
use nvisy_core::Result;

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
