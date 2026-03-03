//! Audio transcription operation.

use crate::operation::Operation;
use nvisy_core::Error;

/// Transcribes audio content into text.
pub struct Transcription;

impl Operation for Transcription {
    type Input = ();
    type Output = ();
    type Context = ();

    async fn call(&self, _input: Self::Input, _ctx: Self::Context) -> Result<Self::Output, Error> {
        todo!("Transcription operation not yet implemented")
    }
}
