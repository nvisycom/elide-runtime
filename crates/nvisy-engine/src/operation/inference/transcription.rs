//! Audio transcription operation.

use nvisy_core::Result;

use crate::operation::{Operation, ParallelContext};

const TARGET: &str = "nvisy_engine::op::transcription";

/// Transcribes audio content into text.
pub struct Transcription;

impl Transcription {
    async fn transcribe(&self, _data: ()) -> Result<()> {
        tracing::debug!(target: TARGET, "transcribing audio");
        todo!("Transcription operation not yet implemented")
    }
}

impl Operation for Transcription {
    type Input = ParallelContext;
    type Output = ParallelContext;

    async fn call(&self, input: Self::Input) -> Result<Self::Output> {
        input.parallel_map(|data| self.transcribe(data)).await
    }
}
