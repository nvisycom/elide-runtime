//! Object detection layer for images.
//!
//! Delegates to an [`ObjectBackend`] to detect objects in images,
//! producing entities with [`ImageLocation`] bounding boxes.

use std::str::FromStr;

use serde_json::Value;

use nvisy_codec::handler::{ImageData, Span};
use nvisy_core::data::{EntityCategory, EntityKind};
use nvisy_core::math::BoundingBox;
use nvisy_core::Error;

use crate::{DetectionMethod, Entity, ImageLocation, Location};
use crate::{ParallelContext, DetectionService};

/// Backend trait for object detection providers.
#[async_trait::async_trait]
pub trait ObjectBackend: Send + Sync + 'static {
    /// Detect objects in an image, returning raw JSON dicts.
    ///
    /// Each dict should contain: `label`, `confidence`, `x`, `y`, `width`, `height`,
    /// and optionally `category` and `entity_type`.
    async fn detect_objects(
        &self,
        image_data: &[u8],
        mime_type: &str,
    ) -> Result<Vec<Value>, Error>;
}

/// Object detection layer — delegates to an [`ObjectBackend`] at runtime.
pub struct ObjectDetection<B> {
    backend: B,
}

impl<B: ObjectBackend> ObjectDetection<B> {
    /// Create a new object detection layer with the given backend.
    pub fn new(backend: B) -> Self {
        Self { backend }
    }
}

#[async_trait::async_trait]
impl<B: ObjectBackend> DetectionService<(), ImageData> for ObjectDetection<B> {
    type Context = ParallelContext;

    async fn detect(
        &self,
        spans: Vec<Span<(), ImageData>>,
    ) -> Result<Vec<Entity>, Error> {
        let mut entities = Vec::new();

        for span in &spans {
            let png_bytes = span.data.encode_png()?;

            let raw = self.backend.detect_objects(&png_bytes, "image/png").await?;

            for item in &raw {
                let obj = item.as_object().ok_or_else(|| {
                    Error::python("Expected JSON object in object detection results".to_string())
                })?;

                let label = obj
                    .get("label")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown");

                let entity_kind = obj
                    .get("entity_type")
                    .and_then(Value::as_str)
                    .and_then(|s| EntityKind::from_str(s).ok())
                    .unwrap_or(EntityKind::Logo);

                let category = obj
                    .get("category")
                    .and_then(Value::as_str)
                    .map(|s| match s {
                        "pii" => EntityCategory::Pii,
                        "phi" => EntityCategory::Phi,
                        "biometric" => EntityCategory::Biometric,
                        other => EntityCategory::Custom(other.to_string()),
                    })
                    .unwrap_or(EntityCategory::Pii);

                let confidence = obj.get("confidence").and_then(Value::as_f64).unwrap_or(0.0);
                let x = obj.get("x").and_then(Value::as_f64).unwrap_or(0.0);
                let y = obj.get("y").and_then(Value::as_f64).unwrap_or(0.0);
                let width = obj.get("width").and_then(Value::as_f64).unwrap_or(0.0);
                let height = obj.get("height").and_then(Value::as_f64).unwrap_or(0.0);

                let entity = Entity::new(
                    category,
                    entity_kind,
                    label,
                    DetectionMethod::ObjectDetection,
                    confidence,
                )
                .with_location(Location::Image(ImageLocation {
                    bounding_box: BoundingBox { x, y, width, height },
                    image_id: None,
                    page_number: None,
                }))
                .with_parent(&span.source);

                entities.push(entity);
            }
        }

        Ok(entities)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    struct MockObjectBackend;

    #[async_trait::async_trait]
    impl ObjectBackend for MockObjectBackend {
        async fn detect_objects(&self, _: &[u8], _: &str) -> Result<Vec<Value>, Error> {
            Ok(vec![json!({
                "label": "license_plate",
                "entity_type": "license_plate",
                "category": "pii",
                "confidence": 0.88,
                "x": 100.0,
                "y": 200.0,
                "width": 80.0,
                "height": 30.0
            })])
        }
    }

    #[tokio::test]
    async fn detect_object_produces_image_location() {
        let layer = ObjectDetection::new(MockObjectBackend);

        let img = ImageData::new_rgb(400, 300);
        let spans = vec![Span::new((), img)];

        let entities = layer.detect(spans).await.unwrap();
        assert_eq!(entities.len(), 1);
        assert_eq!(entities[0].entity_kind, EntityKind::LicensePlate);
        assert_eq!(entities[0].detection_method, DetectionMethod::ObjectDetection);
        assert_eq!(entities[0].value, "license_plate");

        let loc = entities[0].location.as_ref().unwrap().as_image().unwrap();
        assert!((loc.bounding_box.x - 100.0).abs() < f64::EPSILON);
    }
}
