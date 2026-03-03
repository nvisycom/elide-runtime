//! Summarization operation.

use crate::operation::Operation;
use nvisy_core::Error;

/// Produces a summary of text content.
pub struct Summarization;

impl Operation for Summarization {
    type Input = ();
    type Output = ();
    type Context = ();

    async fn call(&self, _input: Self::Input, _ctx: Self::Context) -> Result<Self::Output, Error> {
        todo!("Summarization operation not yet implemented")
    }
}
