//! OCR text extraction via the Python backend.
//!
//! Calls `nvisy_ai.detect_ocr()` through the Python bridge to perform
//! optical character recognition on images, returning text regions with
//! bounding boxes.

use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};

use nvisy_ontology::ontology::entity::{BoundingBox, Entity, EntityLocation};
use nvisy_core::error::Error;
use nvisy_ontology::ontology::entity::{DetectionMethod, EntityCategory};
use crate::bridge::PythonBridge;
use crate::error::from_pyerr;

/// Configuration for OCR detection.
#[derive(Debug, Clone)]
pub struct OcrConfig {
    /// Language hint (e.g. `"eng"` for English).
    pub language: String,
    /// OCR engine to use (`"tesseract"`, `"google-vision"`, `"aws-textract"`).
    pub engine: String,
    /// Minimum confidence threshold for OCR results.
    pub confidence_threshold: f64,
}

/// Call Python `detect_ocr()` via GIL + `spawn_blocking`.
///
/// Returns a list of entities with `DetectionMethod::Ocr`, each carrying
/// a bounding box indicating where the text was found in the image.
pub async fn detect_ocr(
    bridge: &PythonBridge,
    image_data: &[u8],
    mime_type: &str,
    config: &OcrConfig,
) -> Result<Vec<Entity>, Error> {
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

            parse_ocr_results(result)
        })
    })
    .await
    .map_err(|e| Error::python(format!("Task join error: {}", e)))?
}

/// Parse Python list[dict] OCR response into Vec<Entity>.
///
/// Expected Python response format:
/// ```python
/// [
///     {
///         "text": "John Doe",
///         "x": 100.0,
///         "y": 200.0,
///         "width": 150.0,
///         "height": 30.0,
///         "confidence": 0.95
///     },
///     ...
/// ]
/// ```
fn parse_ocr_results(result: Bound<'_, PyAny>) -> Result<Vec<Entity>, Error> {
    let list: &Bound<'_, PyList> = result.downcast().map_err(|e| {
        Error::python(format!("Expected list from Python OCR: {}", e))
    })?;

    let mut entities = Vec::new();

    for item in list.iter() {
        let dict: &Bound<'_, PyDict> = item.downcast().map_err(|e| {
            Error::python(format!("Expected dict in OCR list: {}", e))
        })?;

        let text: String = dict
            .get_item("text")
            .map_err(from_pyerr)?
            .ok_or_else(|| Error::python("Missing 'text' in OCR result"))?
            .extract()
            .map_err(from_pyerr)?;

        let x: f64 = dict
            .get_item("x")
            .map_err(from_pyerr)?
            .and_then(|v| v.extract().ok())
            .unwrap_or(0.0);

        let y: f64 = dict
            .get_item("y")
            .map_err(from_pyerr)?
            .and_then(|v| v.extract().ok())
            .unwrap_or(0.0);

        let width: f64 = dict
            .get_item("width")
            .map_err(from_pyerr)?
            .and_then(|v| v.extract().ok())
            .unwrap_or(0.0);

        let height: f64 = dict
            .get_item("height")
            .map_err(from_pyerr)?
            .and_then(|v| v.extract().ok())
            .unwrap_or(0.0);

        let confidence: f64 = dict
            .get_item("confidence")
            .map_err(from_pyerr)?
            .and_then(|v| v.extract().ok())
            .unwrap_or(0.0);

        let entity = Entity::new(
            EntityCategory::Pii,
            "ocr_text",
            &text,
            DetectionMethod::Ocr,
            confidence,
            EntityLocation {
                start_offset: 0,
                end_offset: text.len(),
                element_id: None,
                page_number: None,
                bounding_box: Some(BoundingBox { x, y, width, height }),
                row_index: None,
                column_index: None,
                image_id: None,
            },
        );

        entities.push(entity);
    }

    Ok(entities)
}
