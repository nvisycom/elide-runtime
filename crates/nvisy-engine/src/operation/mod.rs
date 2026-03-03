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

#[allow(dead_code)]
pub(crate) mod inference;
#[allow(dead_code)]
pub(crate) mod lifecycle;
pub(crate) mod processing;

use std::future::Future;

use nvisy_core::Error;

/// A single unit of work in the redaction pipeline.
///
/// Operations are stateless and composable. The engine calls [`Operation::call`]
/// with an input value and a context, and the operation produces a typed output
/// or an error.
pub trait Operation {
    /// Data consumed by this operation.
    type Input;
    /// Data produced by this operation.
    type Output;
    /// Ambient state available during execution (connections, config, etc.).
    type Context;

    /// Execute the operation.
    fn call(
        &self,
        input: Self::Input,
        ctx: Self::Context,
    ) -> impl Future<Output = Result<Self::Output, Error>> + Send;
}
