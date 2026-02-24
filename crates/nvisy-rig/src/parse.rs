//! LLM result parsing.

use std::str::FromStr;

use serde_json::Value;

use nvisy_core::Error;
use nvisy_ontology::entity::{DetectionMethod, Entity, EntityCategory, EntityKind};
use nvisy_ontology::location::{Location, TextLocation};

/// Parse raw JSON dicts from an LLM backend into [`Entity`] values.
///
/// Expected dict keys: `category`, `entity_type`, `value`, `confidence`,
/// and optionally `start_offset` / `end_offset`.
pub fn parse_llm_entities(raw: &[Value]) -> Result<Vec<Entity>, Error> {
    let mut entities = Vec::new();

    for item in raw {
        let obj = item.as_object().ok_or_else(|| {
            Error::validation("Expected JSON object in LLM results".to_string(), "llm-parse")
        })?;

        let category_str = obj
            .get("category")
            .and_then(Value::as_str)
            .ok_or_else(|| Error::validation("Missing 'category'".to_string(), "llm-parse"))?;

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
            .ok_or_else(|| Error::validation("Missing 'entity_type'".to_string(), "llm-parse"))?;

        let entity_kind = match EntityKind::from_str(entity_type_str) {
            Ok(ek) => ek,
            Err(_) => {
                tracing::warn!(entity_type = entity_type_str, "unknown entity type from LLM, dropping");
                continue;
            }
        };

        let value = obj
            .get("value")
            .and_then(Value::as_str)
            .ok_or_else(|| Error::validation("Missing 'value'".to_string(), "llm-parse"))?;

        let confidence = obj
            .get("confidence")
            .and_then(Value::as_f64)
            .ok_or_else(|| Error::validation("Missing 'confidence'".to_string(), "llm-parse"))?;

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
            DetectionMethod::ContextualNlp,
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
