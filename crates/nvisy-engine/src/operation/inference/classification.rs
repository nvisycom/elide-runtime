//! Content classification operation.

use nvisy_core::Result;

use crate::operation::{Operation, ParallelContext};

#[allow(dead_code)]
const TARGET: &str = "nvisy_engine::op::classification";

/// Classifies content by sensitivity, topic, or type.
pub struct Classification;

impl Operation for Classification {
    type Input = ParallelContext;
    type Output = ParallelContext;

    async fn call(&self, _input: Self::Input) -> Result<Self::Output> {
        todo!("Classification operation not yet implemented")
    }
}
