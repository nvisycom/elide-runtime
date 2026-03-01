//! [`OcrBackend`] implementation for [`PythonBridge`].

use serde_json::Value;

use nvisy_core::Error;
use nvisy_core::math::BoundingBox;
use nvisy_python::bridge::PythonBridge;
use nvisy_python::ocr::OcrParams;

use crate::backend::{OcrBackend, OcrConfig, OcrRegion};

/// Converts [`OcrConfig`] to [`OcrParams`] and delegates to `nvisy_python::ocr`.
///
/// Raw JSON dicts from the Python bridge are parsed into typed
/// [`OcrRegion`] values. Expected dict keys: `text`, `x`, `y`,
/// `width`, `height`, `confidence`.
#[async_trait::async_trait]
impl OcrBackend for PythonBridge {
    async fn detect_ocr(
        &self,
        image_data: &[u8],
        mime_type: &str,
        config: &OcrConfig,
    ) -> Result<Vec<OcrRegion>, Error> {
        let params = OcrParams {
            language: config.language.clone(),
            engine: config.engine.clone(),
            confidence_threshold: config.confidence_threshold,
        };
        let raw = nvisy_python::ocr::detect_ocr(self, image_data, mime_type, &params).await?;
        raw.iter().map(parse_region).collect()
    }
}

/// Parse a single raw JSON dict into an [`OcrRegion`].
fn parse_region(item: &Value) -> Result<OcrRegion, Error> {
    let obj = item
        .as_object()
        .ok_or_else(|| Error::runtime("expected JSON object in OCR results", "python", false))?;

    let text = obj
        .get("text")
        .and_then(Value::as_str)
        .ok_or_else(|| Error::runtime("missing 'text' in OCR result", "python", false))?
        .to_owned();

    let x = obj.get("x").and_then(Value::as_f64).unwrap_or(0.0);
    let y = obj.get("y").and_then(Value::as_f64).unwrap_or(0.0);
    let width = obj.get("width").and_then(Value::as_f64).unwrap_or(0.0);
    let height = obj.get("height").and_then(Value::as_f64).unwrap_or(0.0);
    let confidence = obj.get("confidence").and_then(Value::as_f64).unwrap_or(0.0);

    Ok(OcrRegion {
        text,
        confidence,
        bbox: BoundingBox { x, y, width, height },
        polygon: None,
        level: None,
    })
}
