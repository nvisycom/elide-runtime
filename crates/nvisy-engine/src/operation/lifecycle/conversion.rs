//! Content format conversion operation.

use crate::operation::Operation;
use crate::operation::ParallelContext;
use nvisy_core::Error;

/// Converts content between formats.
pub struct Conversion;

impl Operation for Conversion {
    type Input = ParallelContext;
    type Output = ParallelContext;

    async fn call(&self, _input: Self::Input) -> Result<Self::Output, Error> {
        todo!("Conversion operation not yet implemented")
    }
}
