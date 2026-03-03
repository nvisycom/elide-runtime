//! Policy evaluation operation.

use crate::operation::Operation;
use nvisy_core::Error;

/// Evaluates redaction policies against detected entities.
pub struct PolicyEvaluation;

impl Operation for PolicyEvaluation {
    type Input = ();
    type Output = ();
    type Context = ();

    async fn call(&self, _input: Self::Input, _ctx: Self::Context) -> Result<Self::Output, Error> {
        todo!("PolicyEvaluation operation not yet implemented")
    }
}
