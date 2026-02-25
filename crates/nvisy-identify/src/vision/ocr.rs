//! OCR detection layer for images.
//!
//! Wraps an [`OcrBackend`] as a [`DetectionService`] that produces entities
//! with [`ImageLocation`] bounding boxes from OCR text extraction.

use nvisy_codec::handler::{ImageData, Span};
use nvisy_core::Error;
use nvisy_paddle::{OcrBackend, OcrConfig, parse_ocr_entities};

use crate::Entity;
use crate::{ParallelContext, DetectionService};

/// OCR detection layer — delegates to an [`OcrBackend`] at runtime.
///
/// Encodes each image span to PNG and runs OCR to produce text entities
/// with bounding-box locations.
pub struct OcrDetection<B> {
    backend: B,
    config: OcrConfig,
}

impl<B: OcrBackend> OcrDetection<B> {
    /// Create a new OCR detection layer with the given backend and config.
    pub fn new(backend: B, config: OcrConfig) -> Self {
        Self { backend, config }
    }
}

#[async_trait::async_trait]
impl<B: OcrBackend> DetectionService<(), ImageData> for OcrDetection<B> {
    type Context = ParallelContext;

    async fn detect(
        &self,
        spans: Vec<Span<(), ImageData>>,
    ) -> Result<Vec<Entity>, Error> {
        let mut entities = Vec::new();

        for span in &spans {
            let png_bytes = span.data.encode_png()?;

            let raw = self
                .backend
                .detect_ocr(&png_bytes, "image/png", &self.config)
                .await?;

            for entity in parse_ocr_entities(&raw)? {
                entities.push(entity.with_parent(&span.source));
            }
        }

        Ok(entities)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nvisy_ontology::entity::{DetectionMethod, EntityKind};
    use serde_json::{json, Value};

    struct MockOcrBackend;

    #[async_trait::async_trait]
    impl OcrBackend for MockOcrBackend {
        async fn detect_ocr(
            &self,
            _image_data: &[u8],
            _mime_type: &str,
            _config: &OcrConfig,
        ) -> Result<Vec<Value>, Error> {
            Ok(vec![json!({
                "text": "John Doe",
                "x": 10.0,
                "y": 20.0,
                "width": 100.0,
                "height": 30.0,
                "confidence": 0.88
            })])
        }
    }

    #[tokio::test]
    async fn detect_ocr_produces_image_location() {
        let config = OcrConfig {
            language: "eng".into(),
            engine: "tesseract".into(),
            confidence_threshold: 0.5,
        };
        let layer = OcrDetection::new(MockOcrBackend, config);

        let img = ImageData::new_rgb(200, 100);
        let spans = vec![Span::new((), img)];

        let entities = layer.detect(spans).await.unwrap();
        assert_eq!(entities.len(), 1);
        assert_eq!(entities[0].value, "John Doe");
        assert_eq!(entities[0].entity_kind, EntityKind::Handwriting);
        assert_eq!(entities[0].detection_method, DetectionMethod::Ocr);

        let loc = entities[0].location.as_ref().unwrap().as_image().unwrap();
        assert!((loc.bounding_box.x - 10.0).abs() < f64::EPSILON);
        assert!((loc.bounding_box.width - 100.0).abs() < f64::EPSILON);
    }
}
