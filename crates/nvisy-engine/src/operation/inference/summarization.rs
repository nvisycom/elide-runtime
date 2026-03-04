//! Summarization operation.

use crate::operation::Operation;
use crate::operation::ParallelContext;
use nvisy_core::Error;

/// Produces a summary of text content.
pub struct Summarization;

impl Operation for Summarization {
    type Input = ParallelContext;
    type Output = ParallelContext;

    async fn call(&self, _input: Self::Input) -> Result<Self::Output, Error> {
        todo!("Summarization operation not yet implemented")
    }
}
