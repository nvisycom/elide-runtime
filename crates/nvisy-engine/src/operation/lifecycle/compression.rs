//! Content compression operation.

use crate::operation::Operation;
use crate::operation::ParallelContext;
use nvisy_core::Error;

/// Compresses content for storage or transfer.
pub struct Compression;

impl Operation for Compression {
    type Input = ();
    type Output = ();
    type Context = ParallelContext;

    async fn call(&self, _input: Self::Input, _ctx: Self::Context) -> Result<Self::Output, Error> {
        todo!("Compression operation not yet implemented")
    }
}
