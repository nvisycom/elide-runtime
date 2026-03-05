//! Content classification operation.

use nvisy_core::Error;

use crate::operation::{Operation, ParallelContext};

/// Classifies content by sensitivity, topic, or type.
pub struct Classification;

impl Operation for Classification {
    type Input = ParallelContext;
    type Output = ParallelContext;

    async fn call(&self, _input: Self::Input) -> Result<Self::Output, Error> {
        todo!("Classification operation not yet implemented")
    }
}
