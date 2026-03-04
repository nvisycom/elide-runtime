//! Content compression operation.

use crate::operation::Operation;
use crate::operation::ParallelContext;
use nvisy_core::Error;

/// Compresses content for storage or transfer.
pub struct Compression;

impl Operation for Compression {
    type Input = ParallelContext;
    type Output = ParallelContext;

    async fn call(&self, _input: Self::Input) -> Result<Self::Output, Error> {
        todo!("Compression operation not yet implemented")
    }
}
