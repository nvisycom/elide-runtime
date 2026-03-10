//! Content validation operation.

use nvisy_core::Result;

use crate::operation::{Operation, ParallelContext};

#[allow(dead_code)]
const TARGET: &str = "nvisy_engine::op::validation";

/// Validates content integrity or conformance.
pub struct Validation;

impl Operation for Validation {
    type Input = ParallelContext;
    type Output = ParallelContext;

    async fn call(&self, _input: Self::Input) -> Result<Self::Output> {
        todo!("Validation operation not yet implemented")
    }
}
