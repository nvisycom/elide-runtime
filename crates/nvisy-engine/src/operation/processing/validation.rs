//! Content validation operation.

use nvisy_core::Result;

use crate::operation::{Operation, ParallelContext};

const TARGET: &str = "nvisy_engine::op::validation";

/// Validates content integrity or conformance.
pub struct Validation;

impl Validation {
    async fn validate(&self, _data: ()) -> Result<()> {
        tracing::debug!(target: TARGET, "validating content");
        todo!("Validation operation not yet implemented")
    }
}

impl Operation for Validation {
    type Input = ParallelContext;
    type Output = ParallelContext;

    async fn call(&self, input: Self::Input) -> Result<Self::Output> {
        input.parallel_map(|data| self.validate(data)).await
    }
}
