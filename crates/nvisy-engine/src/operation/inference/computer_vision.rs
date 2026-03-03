//! Computer vision operation.

use crate::operation::Operation;
use nvisy_core::Error;

/// Detects visual entities (faces, license plates, etc.) in image content.
pub struct ComputerVision;

impl Operation for ComputerVision {
    type Input = ();
    type Output = ();
    type Context = ();

    async fn call(&self, _input: Self::Input, _ctx: Self::Context) -> Result<Self::Output, Error> {
        todo!("ComputerVision operation not yet implemented")
    }
}
