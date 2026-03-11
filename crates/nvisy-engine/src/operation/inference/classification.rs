//! Content classification operation.

use nvisy_core::Result;

use crate::operation::{Operation, ParallelContext};

const TARGET: &str = "nvisy_engine::op::classification";

/// Classifies content by sensitivity, topic, or type.
pub struct Classification;

impl Classification {
    async fn classify(&self, _data: ()) -> Result<()> {
        tracing::debug!(target: TARGET, "classifying content");
        todo!("Classification operation not yet implemented")
    }
}

impl Operation for Classification {
    type Input = ParallelContext;
    type Output = ParallelContext;

    async fn call(&self, input: Self::Input) -> Result<Self::Output> {
        input.parallel_map(|data| self.classify(data)).await
    }
}
