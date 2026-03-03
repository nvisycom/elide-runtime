//! Adapter bridging [`OcrBackend`] (traditional OCR) → [`OcrProvider`] (VLM agent).

use std::sync::Arc;

use async_trait::async_trait;

use nvisy_ocr::{ImageFormat, ImageInput, OcrBackend, OcrConfig};
use nvisy_rig::agent::{OcrProvider, OcrTextRegion};
use nvisy_rig::error::Error;

/// Adapts an [`OcrBackend`] (traditional OCR) into an [`OcrProvider`]
/// for use with the VLM [`OcrAgent`](nvisy_rig::agent::OcrAgent).
pub struct OcrBackendProvider {
    backend: Arc<dyn OcrBackend>,
    config: OcrConfig,
}

impl OcrBackendProvider {
    /// Create a new provider wrapping the given backend and config.
    pub fn new(backend: Arc<dyn OcrBackend>, config: OcrConfig) -> Self {
        Self { backend, config }
    }
}

#[async_trait]
impl OcrProvider for OcrBackendProvider {
    async fn extract_text(&self, image_data: &[u8]) -> Result<Vec<OcrTextRegion>, Error> {
        let image = ImageInput::new(image_data.to_vec(), ImageFormat::Png);
        let regions = self
            .backend
            .run(&image, &self.config)
            .await?;

        Ok(regions
            .into_iter()
            .map(|r| OcrTextRegion {
                text: r.text,
                confidence: r.confidence,
                bbox: Some(r.bbox),
                polygon: r.polygon,
                level: r.level,
            })
            .collect())
    }
}

/// Extracts text from image content via OCR.
pub struct Ocr;

impl crate::operation::Operation for Ocr {
    type Input = crate::operation::ParallelContext;
    type Output = crate::operation::ParallelContext;

    async fn call(&self, _input: Self::Input) -> Result<Self::Output, nvisy_core::Error> {
        todo!("OCR operation not yet implemented")
    }
}
