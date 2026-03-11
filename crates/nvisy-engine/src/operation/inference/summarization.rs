//! Summarization operation.

use nvisy_core::Result;

use crate::operation::{Operation, ParallelContext};

const TARGET: &str = "nvisy_engine::op::summarization";

/// Produces a summary of text content.
pub struct Summarization;

impl Summarization {
    async fn summarize(&self, _data: ()) -> Result<()> {
        tracing::debug!(target: TARGET, "summarizing content");
        todo!("Summarization operation not yet implemented")
    }
}

impl Operation for Summarization {
    type Input = ParallelContext;
    type Output = ParallelContext;

    async fn call(&self, input: Self::Input) -> Result<Self::Output> {
        input.parallel_map(|data| self.summarize(data)).await
    }
}
