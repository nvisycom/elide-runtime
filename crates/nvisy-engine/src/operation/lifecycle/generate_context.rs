//! Generate a new context from detection results and content data.

use nvisy_core::Result;

use crate::operation::{Operation, ParallelContext};

const TARGET: &str = "nvisy_engine::op::generate_context";

/// Generates a new context from pipeline results.
///
/// Currently a stub — will eventually support summarization,
/// translation, and audit context generation.
pub struct GenerateContext;

impl GenerateContext {
    async fn generate(&self, _data: ()) -> Result<()> {
        tracing::debug!(target: TARGET, "generate context (stub)");
        Ok(())
    }
}

impl Operation for GenerateContext {
    type Input = ParallelContext;
    type Output = ParallelContext;

    async fn call(&self, input: Self::Input) -> Result<Self::Output> {
        input.parallel_map(|data| self.generate(data)).await
    }
}
