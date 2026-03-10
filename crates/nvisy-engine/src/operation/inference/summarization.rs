//! Summarization operation.

use nvisy_core::Result;

use crate::operation::{Operation, ParallelContext};

#[allow(dead_code)]
const TARGET: &str = "nvisy_engine::op::summarization";

/// Produces a summary of text content.
pub struct Summarization;

impl Operation for Summarization {
    type Input = ParallelContext;
    type Output = ParallelContext;

    async fn call(&self, _input: Self::Input) -> Result<Self::Output> {
        todo!("Summarization operation not yet implemented")
    }
}
