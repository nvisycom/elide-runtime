//! Content encryption operation.

use crate::operation::Operation;
use nvisy_core::Error;

/// Encrypts content at rest or in transit.
pub struct Encryption;

impl Operation for Encryption {
    type Input = ();
    type Output = ();
    type Context = ();

    async fn call(&self, _input: Self::Input, _ctx: Self::Context) -> Result<Self::Output, Error> {
        todo!("Encryption operation not yet implemented")
    }
}
