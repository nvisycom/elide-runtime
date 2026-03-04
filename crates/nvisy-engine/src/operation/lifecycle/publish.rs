//! Content publishing operation.

use crate::operation::Operation;
use crate::operation::ParallelContext;
use nvisy_core::Error;

/// Publishes redacted content to a downstream target.
pub struct Publish;

impl Operation for Publish {
    type Input = ParallelContext;
    type Output = ParallelContext;

    async fn call(&self, _input: Self::Input) -> Result<Self::Output, Error> {
        todo!("Publish operation not yet implemented")
    }
}
