//! Content encryption operation.

use nvisy_core::Error;

use crate::operation::{Operation, ParallelContext};

/// Encrypts content at rest or in transit.
pub struct Encryption;

impl Operation for Encryption {
    type Input = ParallelContext;
    type Output = ParallelContext;

    async fn call(&self, _input: Self::Input) -> Result<Self::Output, Error> {
        todo!("Encryption operation not yet implemented")
    }
}
