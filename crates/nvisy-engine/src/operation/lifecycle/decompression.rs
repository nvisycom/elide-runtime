//! Content decompression operation.

use nvisy_core::Result;

use crate::operation::{Operation, ParallelContext};

const TARGET: &str = "nvisy_engine::op::decompression";

/// Decompresses content from storage or transfer.
pub struct Decompression;

impl Decompression {
    async fn decompress(&self, _data: ()) -> Result<()> {
        tracing::debug!(target: TARGET, "decompressing content");
        todo!("Decompression operation not yet implemented")
    }
}

impl Operation for Decompression {
    type Input = ParallelContext;
    type Output = ParallelContext;

    async fn call(&self, input: Self::Input) -> Result<Self::Output> {
        input.parallel_map(|data| self.decompress(data)).await
    }
}
