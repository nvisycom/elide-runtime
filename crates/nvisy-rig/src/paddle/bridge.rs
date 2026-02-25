//! [`OcrBackend`] implementation for [`PythonBridge`].

use serde_json::Value;

use nvisy_core::Error;
use nvisy_python::bridge::PythonBridge;
use nvisy_python::ocr::OcrParams;

use super::backend::{OcrBackend, OcrConfig};

/// Converts [`OcrConfig`] to [`OcrParams`] and delegates to `nvisy_python::ocr`.
#[async_trait::async_trait]
impl OcrBackend for PythonBridge {
    async fn detect_ocr(
        &self,
        image_data: &[u8],
        mime_type: &str,
        config: &OcrConfig,
    ) -> Result<Vec<Value>, Error> {
        let params = OcrParams {
            language: config.language.clone(),
            engine: config.engine.clone(),
            confidence_threshold: config.confidence_threshold,
        };
        nvisy_python::ocr::detect_ocr(self, image_data, mime_type, &params).await
    }
}
