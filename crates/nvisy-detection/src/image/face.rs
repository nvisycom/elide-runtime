//! Face detection layer for images.
//!
//! Delegates to a [`FaceBackend`] to detect human faces in images,
//! producing entities with [`ImageLocation`] bounding boxes.

use image::DynamicImage;
use serde_json::Value;

use nvisy_codec::handler::Span;
use nvisy_core::data::EntityCategory;
use nvisy_core::math::BoundingBox;
use nvisy_core::Error;
use nvisy_core::path::ContentSource;

use crate::{DetectionMethod, Entity, EntityKind, ImageLocation, Location};
use crate::{ParallelContext, Detect};

/// Backend trait for face detection providers.
#[async_trait::async_trait]
pub trait FaceBackend: Send + Sync + 'static {
    /// Detect faces in an image, returning raw JSON dicts.
    ///
    /// Each dict should contain: `confidence`, `x`, `y`, `width`, `height`.
    async fn detect_faces(
        &self,
        image_data: &[u8],
        mime_type: &str,
    ) -> Result<Vec<Value>, Error>;
}

/// Face detection layer — delegates to a [`FaceBackend`] at runtime.
pub struct FaceDetection<B> {
    backend: B,
}

impl<B: FaceBackend> FaceDetection<B> {
    /// Create a new face detection layer with the given backend.
    pub fn new(backend: B) -> Self {
        Self { backend }
    }
}

#[async_trait::async_trait]
impl<B: FaceBackend> Detect<(), DynamicImage> for FaceDetection<B> {
    type Context = ParallelContext;

    async fn detect(
        &self,
        spans: Vec<Span<(), DynamicImage>>,
        source: &ContentSource,
    ) -> Result<Vec<Entity>, Error> {
        let mut entities = Vec::new();

        for span in &spans {
            let mut buf = std::io::Cursor::new(Vec::new());
            span.data
                .write_to(&mut buf, image::ImageFormat::Png)
                .map_err(|e| Error::validation(format!("PNG encode failed: {e}"), "face-detection"))?;
            let png_bytes = buf.into_inner();

            let raw = self.backend.detect_faces(&png_bytes, "image/png").await?;

            for item in &raw {
                let obj = item.as_object().ok_or_else(|| {
                    Error::python("Expected JSON object in face detection results".to_string())
                })?;

                let confidence = obj.get("confidence").and_then(Value::as_f64).unwrap_or(0.0);
                let x = obj.get("x").and_then(Value::as_f64).unwrap_or(0.0);
                let y = obj.get("y").and_then(Value::as_f64).unwrap_or(0.0);
                let width = obj.get("width").and_then(Value::as_f64).unwrap_or(0.0);
                let height = obj.get("height").and_then(Value::as_f64).unwrap_or(0.0);

                let entity = Entity::new(
                    EntityCategory::Biometric,
                    EntityKind::Face,
                    "face",
                    DetectionMethod::FaceDetection,
                    confidence,
                )
                .with_location(Location::Image(ImageLocation {
                    bounding_box: BoundingBox { x, y, width, height },
                    image_id: None,
                    page_number: None,
                }))
                .with_parent(source);

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

    struct MockFaceBackend;

    #[async_trait::async_trait]
    impl FaceBackend for MockFaceBackend {
        async fn detect_faces(&self, _: &[u8], _: &str) -> Result<Vec<Value>, Error> {
            Ok(vec![json!({
                "confidence": 0.98,
                "x": 50.0,
                "y": 30.0,
                "width": 120.0,
                "height": 150.0
            })])
        }
    }

    #[tokio::test]
    async fn detect_face_produces_image_location() {
        let layer = FaceDetection::new(MockFaceBackend);
        let source = ContentSource::new();

        let img = DynamicImage::new_rgb8(200, 200);
        let spans = vec![Span { id: (), data: img }];

        let entities = layer.detect(spans, &source).await.unwrap();
        assert_eq!(entities.len(), 1);
        assert_eq!(entities[0].entity_kind, EntityKind::Face);
        assert_eq!(entities[0].detection_method, DetectionMethod::FaceDetection);

        let loc = entities[0].location.as_ref().unwrap().as_image().unwrap();
        assert!((loc.bounding_box.x - 50.0).abs() < f64::EPSILON);
        assert!((loc.bounding_box.width - 120.0).abs() < f64::EPSILON);
    }
}
