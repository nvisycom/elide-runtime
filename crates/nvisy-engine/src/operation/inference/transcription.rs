//! Audio transcription operation.

use crate::operation::Operation;
use crate::operation::ParallelContext;
use nvisy_core::Error;

/// Transcribes audio content into text.
pub struct Transcription;

impl Operation for Transcription {
    type Input = ParallelContext;
    type Output = ParallelContext;

    async fn call(&self, _input: Self::Input) -> Result<Self::Output, Error> {
        todo!("Transcription operation not yet implemented")
    }
}
