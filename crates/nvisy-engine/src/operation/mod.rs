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

pub use context::{OperationContext, ParallelContext, SequentialContext};

use std::future::Future;

use nvisy_core::Error;

/// A single unit of work in the redaction pipeline.
///
/// Operations are stateless and composable. The engine calls [`Operation::call`]
/// with an input value and a context, and the operation produces a typed output
/// or an error.
///
/// The `Context` associated type must implement [`OperationContext`], ensuring
/// every operation declares whether it uses [`ParallelContext`] or
/// [`SequentialContext`] processing.
pub trait Operation {
    /// Data consumed by this operation.
    type Input;
    /// Data produced by this operation.
    type Output;
    /// Processing strategy context — must be [`ParallelContext`] or [`SequentialContext`].
    type Context: OperationContext;

    /// Execute the operation.
    fn call(
        &self,
        input: Self::Input,
        ctx: Self::Context,
    ) -> impl Future<Output = Result<Self::Output, Error>> + Send;
}
