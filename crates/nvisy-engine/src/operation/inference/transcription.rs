//! Audio transcription operation.

use nvisy_core::Result;

use crate::operation::{Operation, ParallelContext};

#[allow(dead_code)]
const TARGET: &str = "nvisy_engine::op::transcription";

/// Transcribes audio content into text.
pub struct Transcription;

impl Operation for Transcription {
    type Input = ParallelContext;
    type Output = ParallelContext;

    async fn call(&self, _input: Self::Input) -> Result<Self::Output> {
        todo!("Transcription operation not yet implemented")
    }
}
