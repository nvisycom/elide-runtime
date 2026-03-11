//! Translation operation.

use nvisy_core::Result;

use crate::operation::{Operation, ParallelContext};

const TARGET: &str = "nvisy_engine::op::translation";

/// Translates text content between languages.
pub struct Translation;

impl Translation {
    async fn translate(&self, _data: ()) -> Result<()> {
        tracing::debug!(target: TARGET, "translating content");
        todo!("Translation operation not yet implemented")
    }
}

impl Operation for Translation {
    type Input = ParallelContext;
    type Output = ParallelContext;

    async fn call(&self, input: Self::Input) -> Result<Self::Output> {
        input.parallel_map(|data| self.translate(data)).await
    }
}
