//! Content classification operation.

use crate::operation::Operation;
use crate::operation::ParallelContext;
use nvisy_core::Error;

/// Classifies content by sensitivity, topic, or type.
pub struct Classification;

impl Operation for Classification {
    type Input = ParallelContext;
    type Output = ParallelContext;

    async fn call(&self, _input: Self::Input) -> Result<Self::Output, Error> {
        todo!("Classification operation not yet implemented")
    }
}
