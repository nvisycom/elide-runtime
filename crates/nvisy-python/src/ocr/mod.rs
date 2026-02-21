//! OCR text extraction via the Python backend.
//!
//! Calls `nvisy_ai.detect_ocr()` through the Python bridge to perform
//! optical character recognition on images, returning raw JSON values.
//! Entity construction is handled by the pipeline's `OcrBackend` /
//! `GenerateOcrAction` layer.

use pyo3::prelude::*;
use pyo3::types::PyDict;
use serde_json::Value;

use nvisy_core::Error;
use crate::bridge::{PythonBridge, from_pyerr};

/// Parameters for OCR detection, independent of any pipeline types.
#[derive(Debug, Clone)]
pub struct OcrParams {
    /// Language hint (e.g. `"eng"` for English).
    pub language: String,
    /// OCR engine to use (`"tesseract"`, `"google-vision"`, `"aws-textract"`).
    pub engine: String,
    /// Minimum confidence threshold for OCR results.
    pub confidence_threshold: f64,
}

/// Call Python `detect_ocr()` synchronously via `spawn_blocking`.
///
/// Returns raw JSON dicts — no domain-type construction.
pub async fn detect_ocr(
    bridge: &PythonBridge,
    image_data: &[u8],
    mime_type: &str,
    params: &OcrParams,
) -> Result<Vec<Value>, Error> {
    let image_data = image_data.to_vec();
    let mime_type = mime_type.to_string();
    let params = params.clone();

    bridge
        .call_sync("detect_ocr", move |py| {
            let kwargs = PyDict::new(py);
            kwargs.set_item("image_bytes", &image_data[..]).map_err(from_pyerr)?;
            kwargs.set_item("mime_type", &mime_type).map_err(from_pyerr)?;
            kwargs.set_item("language", &params.language).map_err(from_pyerr)?;
            kwargs.set_item("engine", &params.engine).map_err(from_pyerr)?;
            kwargs.set_item("confidence_threshold", params.confidence_threshold).map_err(from_pyerr)?;
            Ok(kwargs)
        })
        .await
}

/// Call Python `detect_ocr()` as a **coroutine** (async Python function).
///
/// Returns raw JSON dicts — no domain-type construction.
pub async fn detect_ocr_async(
    bridge: &PythonBridge,
    image_data: &[u8],
    mime_type: &str,
    params: &OcrParams,
) -> Result<Vec<Value>, Error> {
    let image_data = image_data.to_vec();
    let mime_type = mime_type.to_string();
    let params = params.clone();

    bridge
        .call_async("detect_ocr", move |py| {
            let kwargs = PyDict::new(py);
            kwargs.set_item("image_bytes", &image_data[..]).map_err(from_pyerr)?;
            kwargs.set_item("mime_type", &mime_type).map_err(from_pyerr)?;
            kwargs.set_item("language", &params.language).map_err(from_pyerr)?;
            kwargs.set_item("engine", &params.engine).map_err(from_pyerr)?;
            kwargs.set_item("confidence_threshold", params.confidence_threshold).map_err(from_pyerr)?;
            Ok(kwargs)
        })
        .await
}
