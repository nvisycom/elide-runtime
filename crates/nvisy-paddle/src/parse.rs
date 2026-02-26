//! OCR result parsing.

use serde_json::Value;

use nvisy_core::math::BoundingBox;
use nvisy_core::Error;
use nvisy_ontology::entity::{DetectionMethod, Entity, EntityCategory, EntityKind};
use nvisy_ontology::location::{ImageLocation, Location};

/// Parse raw JSON dicts from an OCR backend into [`Entity`] values.
///
/// Expected dict keys: `text`, `x`, `y`, `width`, `height`, `confidence`.
pub fn parse_ocr_entities(raw: &[Value]) -> Result<Vec<Entity>, Error> {
    let mut entities = Vec::new();

    for item in raw {
        let obj = item.as_object().ok_or_else(|| {
            Error::python("Expected JSON object in OCR results".to_string())
        })?;

        let text = obj
            .get("text")
            .and_then(Value::as_str)
            .ok_or_else(|| Error::python("Missing 'text' in OCR result".to_string()))?;

        let x = obj.get("x").and_then(Value::as_f64).unwrap_or(0.0);
        let y = obj.get("y").and_then(Value::as_f64).unwrap_or(0.0);
        let width = obj.get("width").and_then(Value::as_f64).unwrap_or(0.0);
        let height = obj.get("height").and_then(Value::as_f64).unwrap_or(0.0);
        let confidence = obj.get("confidence").and_then(Value::as_f64).unwrap_or(0.0);

        let entity = Entity::new(
            EntityCategory::Pii,
            EntityKind::Handwriting,
            text,
            DetectionMethod::Ocr,
            confidence,
        )
        .with_location(Location::Image(ImageLocation {
            bounding_box: BoundingBox { x, y, width, height },
            image_id: None,
            page_number: None,
        }));

        entities.push(entity);
    }

    Ok(entities)
}
