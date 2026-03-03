//! Content validation operation.

use crate::operation::Operation;
use crate::operation::ParallelContext;
use nvisy_core::Error;

/// Validates content integrity or conformance.
pub struct Validation;

impl Operation for Validation {
    type Input = ParallelContext;
    type Output = ParallelContext;

    async fn call(&self, _input: Self::Input) -> Result<Self::Output, Error> {
        todo!("Validation operation not yet implemented")
    }
}
