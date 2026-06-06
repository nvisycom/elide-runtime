//! [`NoopBackend`]: no-op [`SttBackend`] for tests and as the default
//! when no real STT provider is configured.

use nvisy_core::Result;
use nvisy_core::entity::ModelProvenance;

use super::stt_backend::{SttBackend, SttRequest, SttResponse};

const MODEL_NAME: &str = "stt-noop";

/// No-op STT backend: every call returns an empty response.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoopBackend;

#[async_trait::async_trait]
impl SttBackend for NoopBackend {
    fn provenance(&self) -> ModelProvenance {
        ModelProvenance::new(MODEL_NAME.to_owned())
    }

    async fn transcribe(&self, _request: SttRequest<'_>) -> Result<SttResponse> {
        Ok(SttResponse::default())
    }
}
