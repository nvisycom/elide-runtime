//! NER detection on images via [`NerBackend::detect_image`].
//!
//! Encodes the image to PNG, sends it to the NER backend, and returns
//! entities with [`ImageLocation`] bounding boxes.

use std::io::Cursor;
use std::str::FromStr;

use image::DynamicImage;
use serde_json::Value;

use nvisy_codec::handler::Span;
use nvisy_core::data::{EntityCategory, EntityKind};
use nvisy_core::math::BoundingBox;
use nvisy_core::Error;

use crate::{DetectionMethod, Entity, ImageLocation, Location};
use crate::{ParallelContext, DetectionService};
use crate::text::ner::{NerBackend, NerConfig};

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
impl<B: NerBackend> DetectionService<(), DynamicImage> for ImageNerDetection<B> {
    type Context = ParallelContext;

    async fn detect(
        &self,
        spans: Vec<Span<(), DynamicImage>>,
    ) -> Result<Vec<Entity>, Error> {
        let mut entities = Vec::new();

        for span in &spans {
            let mut buf = Cursor::new(Vec::new());
            span.data
                .write_to(&mut buf, image::ImageFormat::Png)
                .map_err(|e| Error::validation(format!("PNG encode failed: {e}"), "image-ner"))?;
            let png_bytes = buf.into_inner();

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

/// Parse a single NER result dict into an [`Entity`] with [`ImageLocation`].
///
/// Expected keys: `category`, `entity_type`, `value`, `confidence`,
/// and optionally bounding box fields `x`, `y`, `width`, `height`.
fn parse_image_ner_entity(item: &Value) -> Result<Option<Entity>, Error> {
    let obj = item.as_object().ok_or_else(|| {
        Error::python("Expected JSON object in image NER results".to_string())
    })?;

    let category_str = obj
        .get("category")
        .and_then(Value::as_str)
        .ok_or_else(|| Error::python("Missing 'category'".to_string()))?;

    let category = match category_str {
        "pii" => EntityCategory::Pii,
        "phi" => EntityCategory::Phi,
        "financial" => EntityCategory::Financial,
        "credentials" => EntityCategory::Credentials,
        other => EntityCategory::Custom(other.to_string()),
    };

    let entity_type_str = obj
        .get("entity_type")
        .and_then(Value::as_str)
        .ok_or_else(|| Error::python("Missing 'entity_type'".to_string()))?;

    let entity_kind = match EntityKind::from_str(entity_type_str) {
        Ok(ek) => ek,
        Err(_) => {
            tracing::warn!(entity_type = entity_type_str, "unknown entity type from image NER, dropping");
            return Ok(None);
        }
    };

    let value = obj
        .get("value")
        .and_then(Value::as_str)
        .ok_or_else(|| Error::python("Missing 'value'".to_string()))?;

    let confidence = obj
        .get("confidence")
        .and_then(Value::as_f64)
        .ok_or_else(|| Error::python("Missing 'confidence'".to_string()))?;

    let x = obj.get("x").and_then(Value::as_f64).unwrap_or(0.0);
    let y = obj.get("y").and_then(Value::as_f64).unwrap_or(0.0);
    let width = obj.get("width").and_then(Value::as_f64).unwrap_or(0.0);
    let height = obj.get("height").and_then(Value::as_f64).unwrap_or(0.0);

    let entity = Entity::new(category, entity_kind, value, DetectionMethod::Ner, confidence)
        .with_location(Location::Image(ImageLocation {
            bounding_box: BoundingBox { x, y, width, height },
            image_id: None,
            page_number: None,
        }));

    Ok(Some(entity))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

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
        let img = DynamicImage::new_rgb8(1, 1);
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
