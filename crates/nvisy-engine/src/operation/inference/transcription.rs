//! Audio transcription operation.

use nvisy_core::Error;

use crate::operation::{Operation, ParallelContext};

/// Transcribes audio content into text.
pub struct Transcription;

impl Operation for Transcription {
    type Input = ParallelContext;
    type Output = ParallelContext;

    async fn call(&self, _input: Self::Input) -> Result<Self::Output, Error> {
        todo!("Transcription operation not yet implemented")
    }
}
