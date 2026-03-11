//! Content format conversion operation.

use nvisy_core::Result;

use crate::operation::{Operation, ParallelContext};

const TARGET: &str = "nvisy_engine::op::conversion";

/// Converts content between formats.
pub struct Conversion;

impl Conversion {
    async fn convert(&self, _data: ()) -> Result<()> {
        tracing::debug!(target: TARGET, "converting format");
        todo!("Conversion operation not yet implemented")
    }
}

impl Operation for Conversion {
    type Input = ParallelContext;
    type Output = ParallelContext;

    async fn call(&self, input: Self::Input) -> Result<Self::Output> {
        input.parallel_map(|data| self.convert(data)).await
    }
}
