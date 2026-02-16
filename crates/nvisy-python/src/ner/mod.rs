//! Named-entity recognition (NER) detection via a Python AI backend.
//!
//! Functions in this module acquire the GIL, call into the Python `nvisy_ai`
//! module, and return raw JSON values. Entity construction is handled by
//! the pipeline's [`NerBackend`] / [`DetectNerAction`] layer.

use pyo3::prelude::*;
use pyo3::types::PyDict;
use serde_json::Value;

use nvisy_core::error::Error;
use nvisy_pipeline::detection::ner::{NerBackend, NerConfig};
use crate::bridge::PythonBridge;
use crate::error::from_pyerr;

/// Call Python `detect_ner()` via GIL + `spawn_blocking`.
///
/// Returns raw JSON dicts — no domain-type construction.
pub async fn detect_ner(
    bridge: &PythonBridge,
    text: &str,
    config: &NerConfig,
) -> Result<Vec<Value>, Error> {
    let module_name = bridge.module_name().to_string();
    let text = text.to_string();
    let config = config.clone();

    tokio::task::spawn_blocking(move || {
        Python::with_gil(|py| {
            let module = py.import(&module_name).map_err(from_pyerr)?;

            let kwargs = PyDict::new(py);
            kwargs.set_item("text", &text).map_err(from_pyerr)?;
            kwargs.set_item("entity_types", &config.entity_types).map_err(from_pyerr)?;
            kwargs.set_item("confidence_threshold", config.confidence_threshold).map_err(from_pyerr)?;

            let result = module
                .call_method("detect_ner", (), Some(&kwargs))
                .map_err(from_pyerr)?;

            pythonize::depythonize::<Vec<Value>>(&result).map_err(|e| {
                Error::python(format!("Failed to deserialize NER result: {}", e))
            })
        })
    })
    .await
    .map_err(|e| Error::python(format!("Task join error: {}", e)))?
}

/// Call Python `detect_ner_image()` via GIL + `spawn_blocking`.
///
/// Returns raw JSON dicts — no domain-type construction.
pub async fn detect_ner_image(
    bridge: &PythonBridge,
    image_data: &[u8],
    mime_type: &str,
    config: &NerConfig,
) -> Result<Vec<Value>, Error> {
    let module_name = bridge.module_name().to_string();
    let image_data = image_data.to_vec();
    let mime_type = mime_type.to_string();
    let config = config.clone();

    tokio::task::spawn_blocking(move || {
        Python::with_gil(|py| {
            let module = py.import(&module_name).map_err(from_pyerr)?;

            let kwargs = PyDict::new(py);
            kwargs.set_item("image_bytes", &image_data[..]).map_err(from_pyerr)?;
            kwargs.set_item("mime_type", &mime_type).map_err(from_pyerr)?;
            kwargs.set_item("entity_types", &config.entity_types).map_err(from_pyerr)?;
            kwargs.set_item("confidence_threshold", config.confidence_threshold).map_err(from_pyerr)?;

            let result = module
                .call_method("detect_ner_image", (), Some(&kwargs))
                .map_err(from_pyerr)?;

            pythonize::depythonize::<Vec<Value>>(&result).map_err(|e| {
                Error::python(format!("Failed to deserialize NER image result: {}", e))
            })
        })
    })
    .await
    .map_err(|e| Error::python(format!("Task join error: {}", e)))?
}

/// [`NerBackend`] implementation for [`PythonBridge`].
///
/// Delegates to the `detect_ner` / `detect_ner_image` functions above.
#[async_trait::async_trait]
impl NerBackend for PythonBridge {
    async fn detect_text(
        &self,
        text: &str,
        config: &NerConfig,
    ) -> Result<Vec<Value>, Error> {
        detect_ner(self, text, config).await
    }

    async fn detect_image(
        &self,
        image_data: &[u8],
        mime_type: &str,
        config: &NerConfig,
    ) -> Result<Vec<Value>, Error> {
        detect_ner_image(self, image_data, mime_type, config).await
    }
}
