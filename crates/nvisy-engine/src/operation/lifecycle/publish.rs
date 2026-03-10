//! Content publishing operation.

use nvisy_core::Result;

use crate::operation::{Operation, ParallelContext};

const TARGET: &str = "nvisy_engine::op::publish";

/// Publishes redacted content to a downstream target.
pub struct Publish;

impl Publish {
    async fn publish(&self, _data: ()) -> Result<()> {
        tracing::debug!(target: TARGET, "publishing content");
        todo!("Publish operation not yet implemented")
    }
}

impl Operation for Publish {
    type Input = ParallelContext;
    type Output = ParallelContext;

    async fn call(&self, input: Self::Input) -> Result<Self::Output> {
        input.parallel_map(|data| self.publish(data)).await
    }
}
