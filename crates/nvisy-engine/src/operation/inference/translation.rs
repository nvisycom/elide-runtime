//! Translation operation.

use crate::operation::Operation;
use crate::operation::ParallelContext;
use nvisy_core::Error;

/// Translates text content between languages.
pub struct Translation;

impl Operation for Translation {
    type Input = ParallelContext;
    type Output = ParallelContext;

    async fn call(&self, _input: Self::Input) -> Result<Self::Output, Error> {
        todo!("Translation operation not yet implemented")
    }
}
