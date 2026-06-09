//! [`NoopBackend`] — returns no OCR results. Default selection
//! for deployments where the runtime should accept image content
//! but isn't expected to recognise text in it.
//!
//! Useful in tests, as a placeholder while wiring up a real
//! externalised backend, and for redaction pipelines that operate
//! purely on metadata or on entities sourced from elsewhere.

use nvisy_core::Error;
use nvisy_core::entity::ModelProvenance;

use super::ocr_backend::{OcrBackend, OcrRequest, OcrResponse};

/// An [`OcrBackend`] that produces no OCR results.
///
/// Every call returns an empty response.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoopBackend;

impl NoopBackend {
    /// Construct an empty backend.
    pub fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl OcrBackend for NoopBackend {
    fn provenance(&self) -> ModelProvenance {
        ModelProvenance::new("noop-ocr")
    }

    async fn extract(&self, _request: OcrRequest<'_>) -> Result<OcrResponse, Error> {
        Ok(OcrResponse::default())
    }
}
