//! NER detection on images via [`NerBackend::detect_image`].
//!
//! Encodes the image to PNG, sends it to the NER backend, and returns
//! entities with [`ImageLocation`] bounding boxes.

use nvisy_codec::handler::{ImageData, Span};
use nvisy_core::Error;

use crate::Entity;
use crate::{ParallelContext, DetectionService};
use crate::ner::{NerBackend, NerConfig, parse_image_ner_entity};

/// NER detection layer for images.
///
/// Encodes each image span to PNG and delegates to an [`NerBackend`]
/// for entity recognition.
pub struct ImageNerDetection<B> {
    backend: B,
    config: NerConfig,
}

impl<B: NerBackend> ImageNerDetection<B> {
    /// Create a new image NER detection layer.
    pub fn new(backend: B, config: NerConfig) -> Self {
        Self { backend, config }
    }
}

#[async_trait::async_trait]
impl<B: NerBackend> DetectionService<(), ImageData> for ImageNerDetection<B> {
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
                .detect_image(&png_bytes, "image/png", &self.config)
                .await?;

            for item in &raw {
                if let Some(entity) = parse_image_ner_entity(item)? {
                    entities.push(entity.with_parent(&span.source));
                }
            }
        }

        Ok(entities)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DetectionMethod;
    use serde_json::{json, Value};

    struct MockImageNerBackend;

    #[async_trait::async_trait]
    impl NerBackend for MockImageNerBackend {
        async fn detect_text(&self, _: &str, _: &NerConfig) -> Result<Vec<Value>, Error> {
            Ok(Vec::new())
        }

        async fn detect_image(
            &self,
            _image_data: &[u8],
            _mime_type: &str,
            _config: &NerConfig,
        ) -> Result<Vec<Value>, Error> {
            Ok(vec![json!({
                "category": "pii",
                "entity_type": "person_name",
                "value": "John Doe",
                "confidence": 0.92,
                "x": 10.0,
                "y": 20.0,
                "width": 100.0,
                "height": 30.0
            })])
        }
    }

    #[tokio::test]
    async fn detect_image_produces_image_location() {
        let config = NerConfig {
            entity_types: vec![],
            confidence_threshold: 0.0,
        };
        let layer = ImageNerDetection::new(MockImageNerBackend, config);

        // Create a tiny 1x1 image.
        let img = ImageData::new_rgb(1, 1);
        let spans = vec![Span::new((), img)];

        let entities = layer.detect(spans).await.unwrap();
        assert_eq!(entities.len(), 1);
        assert_eq!(entities[0].value, "John Doe");
        assert_eq!(entities[0].detection_method, DetectionMethod::Ner);

        let loc = entities[0].location.as_ref().unwrap().as_image().unwrap();
        assert!((loc.bounding_box.x - 10.0).abs() < f64::EPSILON);
        assert!((loc.bounding_box.y - 20.0).abs() < f64::EPSILON);
    }
}
