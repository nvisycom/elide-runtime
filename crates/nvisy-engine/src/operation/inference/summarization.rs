//! Summarization operation.

use nvisy_core::Error;

use crate::operation::{Operation, ParallelContext};

/// Produces a summary of text content.
pub struct Summarization;

impl Operation for Summarization {
    type Input = ParallelContext;
    type Output = ParallelContext;

    async fn call(&self, _input: Self::Input) -> Result<Self::Output, Error> {
        todo!("Summarization operation not yet implemented")
    }
}
