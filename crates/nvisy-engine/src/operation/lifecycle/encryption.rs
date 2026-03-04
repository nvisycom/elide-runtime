//! Content encryption operation.

use crate::operation::Operation;
use crate::operation::ParallelContext;
use nvisy_core::Error;

/// Encrypts content at rest or in transit.
pub struct Encryption;

impl Operation for Encryption {
    type Input = ParallelContext;
    type Output = ParallelContext;

    async fn call(&self, _input: Self::Input) -> Result<Self::Output, Error> {
        todo!("Encryption operation not yet implemented")
    }
}
