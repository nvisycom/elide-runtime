//! Adapter bridging [`Backend`](nvisy_ocr::Backend) (traditional OCR) → [`OcrProvider`] (VLM agent).

use std::sync::Arc;

use async_trait::async_trait;

use nvisy_ocr::{Backend, ImageFormat, ImageInput, RunParams};
use nvisy_rig::agent::{OcrProvider, OcrTextRegion};
use nvisy_rig::error::Error;

/// Adapts an OCR [`Backend`](nvisy_ocr::Backend) into an [`OcrProvider`]
/// for use with the VLM [`OcrAgent`](nvisy_rig::agent::OcrAgent).
pub struct OcrBackendProvider {
    backend: Arc<dyn Backend>,
    params: RunParams,
}

impl OcrBackendProvider {
    /// Create a new provider wrapping the given backend and params.
    pub fn new(backend: Arc<dyn Backend>, params: RunParams) -> Self {
        Self { backend, params }
    }
}

#[async_trait]
impl OcrProvider for OcrBackendProvider {
    async fn extract_text(&self, image_data: &[u8]) -> Result<Vec<OcrTextRegion>, Error> {
        let image = ImageInput::new(image_data.to_vec(), ImageFormat::Png);
        let output = self
            .backend
            .run(&image, &self.params)
            .await?;

        Ok(output
            .into_iter()
            .map(|r| OcrTextRegion {
                text: r.text,
                confidence: r.confidence.unwrap_or(0.0),
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
