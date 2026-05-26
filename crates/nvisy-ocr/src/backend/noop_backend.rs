//! [`NoopOcrBackend`] — returns no OCR results. Default selection
//! for deployments where the runtime should accept image content
//! but isn't expected to recognise text in it.
//!
//! Useful in tests, as a placeholder while wiring up a real
//! externalised backend, and for redaction pipelines that operate
//! purely on metadata or on entities sourced from elsewhere.

use async_trait::async_trait;
use nvisy_core::Error;

use crate::core::{Backend, ImageInput, ImageOutput, OcrParams};

/// A [`Backend`] that produces no OCR results.
///
/// Every call returns an empty [`ImageOutput`] whose `source` is
/// derived from the input image, so downstream provenance still
/// flows.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoopOcrBackend;

impl NoopOcrBackend {
    /// Construct an empty backend.
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Backend for NoopOcrBackend {
    async fn run(&self, _image: &ImageInput, _params: OcrParams<'_>) -> Result<ImageOutput, Error> {
        Ok(ImageOutput::new())
    }
}
