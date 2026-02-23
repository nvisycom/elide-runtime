//! [`NerBackend`] implementation for [`PythonBridge`].

use serde_json::Value;

use nvisy_core::Error;
use nvisy_python::bridge::PythonBridge;
use nvisy_python::ner::NerParams;

use super::backend::{NerBackend, NerConfig};

/// Converts [`NerConfig`] to [`NerParams`] and delegates to `nvisy_python::ner`.
#[async_trait::async_trait]
impl NerBackend for PythonBridge {
    async fn detect_text(
        &self,
        text: &str,
        config: &NerConfig,
    ) -> Result<Vec<Value>, Error> {
        let params = NerParams {
            entity_types: config.entity_types.clone(),
            confidence_threshold: config.confidence_threshold,
        };
        nvisy_python::ner::detect_ner(self, text, &params).await
    }

    async fn detect_image(
        &self,
        image_data: &[u8],
        mime_type: &str,
        config: &NerConfig,
    ) -> Result<Vec<Value>, Error> {
        let params = NerParams {
            entity_types: config.entity_types.clone(),
            confidence_threshold: config.confidence_threshold,
        };
        nvisy_python::ner::detect_ner_image(self, image_data, mime_type, &params).await
    }
}
