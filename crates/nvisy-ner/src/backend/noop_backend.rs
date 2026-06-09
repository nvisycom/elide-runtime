//! [`NoopBackend`]: no-op [`NerBackend`] for tests + the default
//! `NerBackend::Noop` config variant in `nvisy-engine`.

use nvisy_core::Result;

use super::ner_backend::{NerBackend, NerRequest, NerResponse};

/// No-op NER backend: every call returns an empty response.
///
/// Useful as a test stub and as the default `[detection.ner]` backend
/// when the operator wants the NER recognizer wired but isn't ready to
/// configure a real backend.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoopBackend;

#[async_trait::async_trait]
impl NerBackend for NoopBackend {
    async fn predict(&self, _request: NerRequest<'_>) -> Result<NerResponse> {
        Ok(NerResponse::default())
    }
}
