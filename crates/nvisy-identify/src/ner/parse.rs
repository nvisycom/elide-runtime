//! NER result parsing for text and image modalities.

use std::str::FromStr;

use serde_json::Value;

use nvisy_core::data::{EntityCategory, EntityKind};
use nvisy_core::math::BoundingBox;
use nvisy_core::Error;

use crate::{DetectionMethod, Entity, ImageLocation, Location, TextLocation};

/// Parse raw JSON dicts from an NER backend into [`Entity`] values.
///
/// Expected dict keys: `category`, `entity_type`, `value`, `confidence`,
/// and optionally `start_offset` / `end_offset`.
pub fn parse_ner_entities(raw: &[Value]) -> Result<Vec<Entity>, Error> {
    let mut entities = Vec::new();

    for item in raw {
        let obj = item.as_object().ok_or_else(|| {
            Error::python("Expected JSON object in NER results".to_string())
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
                tracing::warn!(entity_type = entity_type_str, "unknown entity type from NER, dropping");
                continue;
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

        let start_offset = obj
            .get("start_offset")
            .and_then(Value::as_u64)
            .map(|v| v as usize)
            .unwrap_or(0);

        let end_offset = obj
            .get("end_offset")
            .and_then(Value::as_u64)
            .map(|v| v as usize)
            .unwrap_or(0);

        let entity = Entity::new(
            category,
            entity_kind,
            value,
            DetectionMethod::Ner,
            confidence,
        )
        .with_location(Location::Text(TextLocation {
            start_offset,
            end_offset,
            ..Default::default()
        }));

        entities.push(entity);
    }

    Ok(entities)
}

/// Parse a single NER result dict into an [`Entity`] with [`ImageLocation`].
///
/// Expected keys: `category`, `entity_type`, `value`, `confidence`,
/// and optionally bounding box fields `x`, `y`, `width`, `height`.
pub fn parse_image_ner_entity(item: &Value) -> Result<Option<Entity>, Error> {
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
