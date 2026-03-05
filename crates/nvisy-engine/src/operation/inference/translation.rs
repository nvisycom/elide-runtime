//! Translation operation.

use nvisy_core::Error;

use crate::operation::{Operation, ParallelContext};

/// Translates text content between languages.
pub struct Translation;

impl Operation for Translation {
    type Input = ParallelContext;
    type Output = ParallelContext;

    async fn call(&self, _input: Self::Input) -> Result<Self::Output, Error> {
        todo!("Translation operation not yet implemented")
    }
}
