//! Pattern matching operation.

use crate::operation::Operation;
use nvisy_core::Error;

/// Matches content against regex or literal patterns.
pub struct PatternMatch;

impl Operation for PatternMatch {
    type Input = ();
    type Output = ();
    type Context = ();

    async fn call(&self, _input: Self::Input, _ctx: Self::Context) -> Result<Self::Output, Error> {
        todo!("PatternMatch operation not yet implemented")
    }
}
