//! Content publishing operation.

use nvisy_core::Result;

use crate::operation::{Operation, ParallelContext};

#[allow(dead_code)]
const TARGET: &str = "nvisy_engine::op::publish";

/// Publishes redacted content to a downstream target.
pub struct Publish;

impl Operation for Publish {
    type Input = ParallelContext;
    type Output = ParallelContext;

    async fn call(&self, _input: Self::Input) -> Result<Self::Output> {
        todo!("Publish operation not yet implemented")
    }
}
