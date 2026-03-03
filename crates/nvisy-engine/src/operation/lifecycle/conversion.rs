//! Content format conversion operation.

use crate::operation::Operation;
use crate::operation::ParallelContext;
use nvisy_core::Error;

/// Converts content between formats.
pub struct Conversion;

impl Operation for Conversion {
    type Input = ();
    type Output = ();
    type Context = ParallelContext;

    async fn call(&self, _input: Self::Input, _ctx: Self::Context) -> Result<Self::Output, Error> {
        todo!("Conversion operation not yet implemented")
    }
}
