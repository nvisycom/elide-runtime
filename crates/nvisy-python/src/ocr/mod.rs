//! OCR text extraction via the Python backend.
//!
//! Calls `nvisy_ai.detect_ocr()` through the Python bridge to perform
//! optical character recognition on images, returning raw JSON values.
//! Entity construction is handled by the pipeline's [`OcrBackend`] /
//! [`GenerateOcrAction`] layer.

use pyo3::prelude::*;
use pyo3::types::PyDict;
use serde_json::Value;

use nvisy_core::error::Error;
use crate::bridge::PythonBridge;
use crate::error::from_pyerr;

use nvisy_pipeline::generation::ocr::{OcrBackend, OcrConfig};

/// Call Python `detect_ocr()` via GIL + `spawn_blocking`.
///
/// Returns raw JSON dicts — no domain-type construction.
pub async fn detect_ocr(
    bridge: &PythonBridge,
    image_data: &[u8],
    mime_type: &str,
    config: &OcrConfig,
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
            kwargs.set_item("language", &config.language).map_err(from_pyerr)?;
            kwargs.set_item("engine", &config.engine).map_err(from_pyerr)?;
            kwargs.set_item("confidence_threshold", config.confidence_threshold).map_err(from_pyerr)?;

            let result = module
                .call_method("detect_ocr", (), Some(&kwargs))
                .map_err(from_pyerr)?;

            pythonize::depythonize::<Vec<Value>>(&result).map_err(|e| {
                Error::python(format!("Failed to deserialize OCR result: {}", e))
            })
        })
    })
    .await
    .map_err(|e| Error::python(format!("Task join error: {}", e)))?
}

/// [`OcrBackend`] implementation for [`PythonBridge`].
///
/// Delegates to the `detect_ocr` function above.
#[async_trait::async_trait]
impl OcrBackend for PythonBridge {
    async fn detect_ocr(
        &self,
        image_data: &[u8],
        mime_type: &str,
        config: &OcrConfig,
    ) -> Result<Vec<Value>, Error> {
        self::detect_ocr(self, image_data, mime_type, config).await
    }
}
