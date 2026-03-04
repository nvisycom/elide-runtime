//! Verification operation wrapping [`OcrAgent`](nvisy_rig::agent::OcrAgent).

/// Verifies NER-detected entities against the original image via VLM.
pub struct Verification;

impl crate::operation::Operation for Verification {
    type Input = crate::operation::ParallelContext;
    type Output = crate::operation::ParallelContext;

    async fn call(&self, _input: Self::Input) -> Result<Self::Output, nvisy_core::Error> {
        todo!("Verification operation not yet implemented")
    }
}
