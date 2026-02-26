//! LLM completion response parsing.
//!
//! [`ResponseParser`] extracts text from rig-core completion responses
//! and deserializes JSON (handling markdown fences and empty responses).
//! [`EntityParser`] converts raw JSON dicts into [`Entity`] values.
//!
//! [`Entity`]: nvisy_ontology::entity::Entity

use std::borrow::Cow;
use std::str::FromStr;

use serde::de::DeserializeOwned;
use serde_json::Value;

use rig::completion::{AssistantContent, CompletionResponse};

use nvisy_ontology::entity::{DetectionMethod, Entity, EntityCategory, EntityKind};
use nvisy_ontology::location::{Location, TextLocation};

use crate::error::Error;

/// Thin wrapper around text extracted from an LLM completion response.
pub struct ResponseParser<'a> {
    text: Cow<'a, str>,
}

impl<'a> ResponseParser<'a> {
    /// Extract the text content blocks from a completion response.
    pub fn extract_text<T>(response: &CompletionResponse<T>) -> Result<Self, Error> {
        let texts: Vec<&str> = response
            .choice
            .iter()
            .filter_map(|c| match c {
                AssistantContent::Text(t) => Some(t.text.as_str()),
                _ => None,
            })
            .collect();

        if texts.is_empty() {
            return Err(Error::Response(
                "LLM response contained no text content".to_string(),
            ));
        }

        Ok(Self {
            text: Cow::Owned(texts.join("\n")),
        })
    }

    /// Wrap an already-extracted string.
    pub fn from_text(text: impl Into<Cow<'a, str>>) -> Self {
        Self { text: text.into() }
    }

    pub fn as_str(&self) -> &str {
        &self.text
    }

    pub fn into_string(self) -> String {
        self.text.into_owned()
    }

    /// Deserialize the text as JSON into `T`.
    ///
    /// Strips markdown fences when present. Returns `T::default()` for
    /// empty / `"none"` / `"no entities"` responses.
    pub fn parse_json<T: DeserializeOwned + Default>(&self) -> Result<T, Error> {
        let trimmed = self.text.trim();

        if trimmed.is_empty()
            || trimmed.eq_ignore_ascii_case("none")
            || trimmed.eq_ignore_ascii_case("no entities")
        {
            return Ok(T::default());
        }

        let json_str = extract_fenced_json(trimmed).unwrap_or(trimmed);

        serde_json::from_str::<T>(json_str).map_err(|e| {
            Error::Response(format!(
                "Failed to parse LLM response as JSON: {e}: {}",
                truncate(trimmed, 200),
            ))
        })
    }
}

/// Convert raw JSON dicts (as returned by an LLM) into [`Entity`] values.
///
/// Unknown `entity_type` values are silently dropped — LLMs occasionally
/// hallucinate types that don't exist in the ontology.
pub struct EntityParser;

impl EntityParser {
    /// Parse an array of JSON objects into entities.
    ///
    /// Expected keys: `category`, `entity_type`, `value`, `confidence`,
    /// and optionally `start_offset` / `end_offset`.
    pub fn parse(raw: &[Value]) -> Result<Vec<Entity>, Error> {
        let mut entities = Vec::new();

        for item in raw {
            let obj = item.as_object().ok_or_else(|| {
                Error::Validation("Expected JSON object in LLM results".to_string())
            })?;

            let category_str = obj
                .get("category")
                .and_then(Value::as_str)
                .ok_or_else(|| Error::Validation("Missing 'category'".to_string()))?;

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
                .ok_or_else(|| {
                    Error::Validation("Missing 'entity_type'".to_string())
                })?;

            let entity_kind = match EntityKind::from_str(entity_type_str) {
                Ok(ek) => ek,
                Err(_) => {
                    tracing::warn!(
                        entity_type = entity_type_str,
                        "unknown entity type from LLM, dropping"
                    );
                    continue;
                }
            };

            let value = obj
                .get("value")
                .and_then(Value::as_str)
                .ok_or_else(|| Error::Validation("Missing 'value'".to_string()))?;

            let confidence = obj
                .get("confidence")
                .and_then(Value::as_f64)
                .ok_or_else(|| {
                    Error::Validation("Missing 'confidence'".to_string())
                })?;

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
}

/// Extract JSON content from markdown fences (```` ```json ... ``` ````).
fn extract_fenced_json(text: &str) -> Option<&str> {
    let start_marker = if let Some(pos) = text.find("```json") {
        pos + "```json".len()
    } else if let Some(pos) = text.find("```") {
        pos + "```".len()
    } else {
        return None;
    };

    let rest = &text[start_marker..];
    let rest = rest.strip_prefix('\n').unwrap_or(rest);
    let end = rest.find("```")?;
    let content = rest[..end].trim();

    if content.is_empty() {
        None
    } else {
        Some(content)
    }
}

fn truncate(s: &str, max_len: usize) -> &str {
    if s.len() <= max_len {
        s
    } else {
        let mut end = max_len;
        while end > 0 && !s.is_char_boundary(end) {
            end -= 1;
        }
        &s[..end]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parse_json_raw_array() {
        let text = r#"[{"category":"pii","entity_type":"email_address","value":"a@b.com","confidence":0.9,"start_offset":0,"end_offset":7}]"#;
        let result = ResponseParser::from_text(text).parse_json::<Vec<Value>>().unwrap();
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn parse_json_fenced() {
        let text = "```json\n[{\"category\":\"pii\",\"entity_type\":\"email_address\",\"value\":\"a@b.com\",\"confidence\":0.9}]\n```";
        let result = ResponseParser::from_text(text).parse_json::<Vec<Value>>().unwrap();
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn parse_json_empty_and_sentinel() {
        let empty: Vec<Value> = vec![];
        assert_eq!(ResponseParser::from_text("").parse_json::<Vec<Value>>().unwrap(), empty);
        assert_eq!(ResponseParser::from_text("none").parse_json::<Vec<Value>>().unwrap(), empty);
        assert_eq!(ResponseParser::from_text("No entities").parse_json::<Vec<Value>>().unwrap(), empty);
    }

    #[test]
    fn entity_parser_basic() {
        let raw = vec![json!({
            "category": "credentials",
            "entity_type": "api_key",
            "value": "SECRET",
            "confidence": 0.92,
            "start_offset": 9,
            "end_offset": 15
        })];
        let entities = EntityParser::parse(&raw).unwrap();
        assert_eq!(entities.len(), 1);
        assert_eq!(entities[0].value, "SECRET");
        assert_eq!(entities[0].confidence, 0.92);
    }

    #[test]
    fn entity_parser_unknown_type_skipped() {
        let raw = vec![json!({
            "category": "pii",
            "entity_type": "unknown_thing_xyz",
            "value": "test",
            "confidence": 0.5
        })];
        assert!(EntityParser::parse(&raw).unwrap().is_empty());
    }
}
