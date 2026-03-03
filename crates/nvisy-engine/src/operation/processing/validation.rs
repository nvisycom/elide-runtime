//! Content validation operation.

use crate::operation::Operation;
use nvisy_core::Error;

/// Validates content integrity or conformance.
pub struct Validation;

impl Operation for Validation {
    type Input = ();
    type Output = ();
    type Context = ();

    async fn call(&self, _input: Self::Input, _ctx: Self::Context) -> Result<Self::Output, Error> {
        todo!("Validation operation not yet implemented")
    }
}
