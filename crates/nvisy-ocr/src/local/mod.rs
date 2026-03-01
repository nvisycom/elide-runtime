//! [`OcrBackend`] implementation using oar-ocr (Rust-native PaddleOCR via ONNX Runtime).

use std::io::Cursor;
use std::sync::Arc;

use image::ImageReader;
use oar_ocr::oarocr::ocr::{OAROCR, OAROCRBuilder};

use nvisy_core::Error;
use nvisy_core::math::{Polygon, Vertex};

use crate::backend::{OcrBackend, OcrConfig, OcrRegion};

/// Local OCR backend using PaddleOCR models via ONNX Runtime (oar-ocr).
///
/// Model files (detection ONNX, recognition ONNX, character dictionary)
/// must be provided at construction time.  The Docker image bundles
/// these; for local development set paths via environment variables or
/// constructor arguments.
pub struct LocalOcrBackend {
    engine: Arc<OAROCR>,
}

impl LocalOcrBackend {
    /// Build a new local OCR backend from the given model paths.
    ///
    /// # Errors
    ///
    /// Returns an error if any model file cannot be loaded or if the
    /// ONNX Runtime session fails to initialise.
    pub fn new(det_model: &str, rec_model: &str, char_dict: &str) -> Result<Self, Error> {
        let engine = OAROCRBuilder::new(det_model, rec_model, char_dict)
            .build()
            .map_err(|e| {
                Error::runtime(
                    format!("failed to build oar-ocr engine: {e}"),
                    "local_ocr",
                    false,
                )
            })?;
        Ok(Self {
            engine: Arc::new(engine),
        })
    }
}

#[async_trait::async_trait]
impl OcrBackend for LocalOcrBackend {
    async fn detect_ocr(
        &self,
        image_data: &[u8],
        _mime_type: &str,
        config: &OcrConfig,
    ) -> Result<Vec<OcrRegion>, Error> {
        // Decode image bytes into an RgbImage.
        let rgb = ImageReader::new(Cursor::new(image_data))
            .with_guessed_format()
            .map_err(|e| Error::runtime(format!("image format guess failed: {e}"), "local_ocr", false))?
            .decode()
            .map_err(|e| Error::runtime(format!("image decode failed: {e}"), "local_ocr", false))?
            .to_rgb8();

        // oar-ocr is synchronous — run on a blocking thread.
        let engine = Arc::clone(&self.engine);
        let threshold = config.confidence_threshold;

        let regions = tokio::task::spawn_blocking(move || {
            let results = engine
                .predict(vec![rgb])
                .map_err(|e| Error::runtime(format!("oar-ocr predict failed: {e}"), "local_ocr", false))?;

            let mut out = Vec::new();
            for result in results {
                for tr in result.text_regions {
                    let confidence = tr.confidence.unwrap_or(0.0) as f64;
                    if confidence < threshold {
                        continue;
                    }

                    let text = match tr.text {
                        Some(t) => t.to_string(),
                        None => continue,
                    };

                    // Build polygon from detection points.
                    let polygon = Polygon {
                        vertices: tr
                            .bounding_box
                            .points
                            .iter()
                            .map(|p| Vertex::new(p.x as f64, p.y as f64))
                            .collect(),
                    };

                    // Derive axis-aligned bounding box from polygon.
                    let bbox = polygon.bounding_box();

                    out.push(OcrRegion {
                        text,
                        confidence,
                        bbox,
                        polygon: Some(polygon),
                        level: None,
                    });
                }
            }
            Ok::<_, Error>(out)
        })
        .await
        .map_err(|e| Error::runtime(format!("blocking task panicked: {e}"), "local_ocr", false))??;

        Ok(regions)
    }
}
