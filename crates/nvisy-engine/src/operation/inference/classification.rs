//! Content classification operation.

use crate::operation::Operation;
use nvisy_core::Error;

/// Classifies content by sensitivity, topic, or type.
pub struct Classification;

impl Operation for Classification {
    type Input = ();
    type Output = ();
    type Context = ();

    async fn call(&self, _input: Self::Input, _ctx: Self::Context) -> Result<Self::Output, Error> {
        todo!("Classification operation not yet implemented")
    }
}
