//! Named-entity recognition (NER) detection via a Python AI backend.
//!
//! Functions in this module call into the Python `nvisy_ai` module via
//! [`PythonBridge`] and return raw JSON values.  Entity construction is
//! handled by the pipeline's `NerBackend` / `DetectNerAction` layer.

use pyo3::prelude::*;
use pyo3::types::PyDict;
use serde_json::Value;

use nvisy_core::Error;
use crate::bridge::{PythonBridge, from_pyerr};

/// Parameters for NER detection, independent of any pipeline types.
#[derive(Debug, Clone)]
pub struct NerParams {
    /// Entity type labels to detect (e.g., `["PERSON", "SSN"]`).
    pub entity_types: Vec<String>,
    /// Minimum confidence score to include a detection (0.0 -- 1.0).
    pub confidence_threshold: f64,
}

/// Call Python `detect_ner()` synchronously via `spawn_blocking`.
///
/// Returns raw JSON dicts — no domain-type construction.
pub async fn detect_ner(
    bridge: &PythonBridge,
    text: &str,
    params: &NerParams,
) -> Result<Vec<Value>, Error> {
    let text = text.to_string();
    let params = params.clone();

    bridge
        .call_sync("detect_ner", move |py| {
            let kwargs = PyDict::new(py);
            kwargs.set_item("text", &text).map_err(from_pyerr)?;
            kwargs.set_item("entity_types", &params.entity_types).map_err(from_pyerr)?;
            kwargs.set_item("confidence_threshold", params.confidence_threshold).map_err(from_pyerr)?;
            Ok(kwargs)
        })
        .await
}

/// Call Python `detect_ner_image()` synchronously via `spawn_blocking`.
///
/// Returns raw JSON dicts — no domain-type construction.
pub async fn detect_ner_image(
    bridge: &PythonBridge,
    image_data: &[u8],
    mime_type: &str,
    params: &NerParams,
) -> Result<Vec<Value>, Error> {
    let image_data = image_data.to_vec();
    let mime_type = mime_type.to_string();
    let params = params.clone();

    bridge
        .call_sync("detect_ner_image", move |py| {
            let kwargs = PyDict::new(py);
            kwargs.set_item("image_bytes", &image_data[..]).map_err(from_pyerr)?;
            kwargs.set_item("mime_type", &mime_type).map_err(from_pyerr)?;
            kwargs.set_item("entity_types", &params.entity_types).map_err(from_pyerr)?;
            kwargs.set_item("confidence_threshold", params.confidence_threshold).map_err(from_pyerr)?;
            Ok(kwargs)
        })
        .await
}

/// Call Python `detect_ner()` as a **coroutine** (async Python function).
///
/// Returns raw JSON dicts — no domain-type construction.
pub async fn detect_ner_async(
    bridge: &PythonBridge,
    text: &str,
    params: &NerParams,
) -> Result<Vec<Value>, Error> {
    let text = text.to_string();
    let params = params.clone();

    bridge
        .call_async("detect_ner", move |py| {
            let kwargs = PyDict::new(py);
            kwargs.set_item("text", &text).map_err(from_pyerr)?;
            kwargs.set_item("entity_types", &params.entity_types).map_err(from_pyerr)?;
            kwargs.set_item("confidence_threshold", params.confidence_threshold).map_err(from_pyerr)?;
            Ok(kwargs)
        })
        .await
}

/// Call Python `detect_ner_image()` as a **coroutine** (async Python function).
///
/// Returns raw JSON dicts — no domain-type construction.
pub async fn detect_ner_image_async(
    bridge: &PythonBridge,
    image_data: &[u8],
    mime_type: &str,
    params: &NerParams,
) -> Result<Vec<Value>, Error> {
    let image_data = image_data.to_vec();
    let mime_type = mime_type.to_string();
    let params = params.clone();

    bridge
        .call_async("detect_ner_image", move |py| {
            let kwargs = PyDict::new(py);
            kwargs.set_item("image_bytes", &image_data[..]).map_err(from_pyerr)?;
            kwargs.set_item("mime_type", &mime_type).map_err(from_pyerr)?;
            kwargs.set_item("entity_types", &params.entity_types).map_err(from_pyerr)?;
            kwargs.set_item("confidence_threshold", params.confidence_threshold).map_err(from_pyerr)?;
            Ok(kwargs)
        })
        .await
}
