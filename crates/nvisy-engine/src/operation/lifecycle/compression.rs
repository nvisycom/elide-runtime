//! Content compression operation.

use nvisy_core::Result;

use crate::operation::{Operation, ParallelContext};

const TARGET: &str = "nvisy_engine::op::compression";

/// Compresses content for storage or transfer.
pub struct Compression;

impl Compression {
    async fn compress(&self, _data: ()) -> Result<()> {
        tracing::debug!(target: TARGET, "compressing content");
        todo!("Compression operation not yet implemented")
    }
}

impl Operation for Compression {
    type Input = ParallelContext;
    type Output = ParallelContext;

    async fn call(&self, input: Self::Input) -> Result<Self::Output> {
        input.parallel_map(|data| self.compress(data)).await
    }
}
