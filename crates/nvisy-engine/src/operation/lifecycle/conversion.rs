//! Content format conversion operation.

use nvisy_core::Error;

use crate::operation::{Operation, ParallelContext};

/// Converts content between formats.
pub struct Conversion;

impl Operation for Conversion {
    type Input = ParallelContext;
    type Output = ParallelContext;

    async fn call(&self, _input: Self::Input) -> Result<Self::Output, Error> {
        todo!("Conversion operation not yet implemented")
    }
}
