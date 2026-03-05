//! Content compression operation.

use nvisy_core::Error;

use crate::operation::{Operation, ParallelContext};

/// Compresses content for storage or transfer.
pub struct Compression;

impl Operation for Compression {
    type Input = ParallelContext;
    type Output = ParallelContext;

    async fn call(&self, _input: Self::Input) -> Result<Self::Output, Error> {
        todo!("Compression operation not yet implemented")
    }
}
