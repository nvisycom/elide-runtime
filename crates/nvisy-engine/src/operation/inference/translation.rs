//! Translation operation.

use nvisy_core::Result;

use crate::operation::{Operation, ParallelContext};

#[allow(dead_code)]
const TARGET: &str = "nvisy_engine::op::translation";

/// Translates text content between languages.
pub struct Translation;

impl Operation for Translation {
    type Input = ParallelContext;
    type Output = ParallelContext;

    async fn call(&self, _input: Self::Input) -> Result<Self::Output> {
        todo!("Translation operation not yet implemented")
    }
}
