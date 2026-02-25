//! Response parsing for LLM completions.

use std::str::FromStr;

use serde_json::Value;

use rig::completion::{AssistantContent, CompletionResponse};

use nvisy_core::Error;
use nvisy_ontology::entity::{DetectionMethod, Entity, EntityCategory, EntityKind};
use nvisy_ontology::location::{Location, TextLocation};

/// Extracts text and parses JSON from LLM completion responses.
pub struct ResponseParser;

impl ResponseParser {
    /// Extract the first text content from a completion response.
    pub fn extract_text<T>(response: &CompletionResponse<T>) -> Result<String, Error> {
        let texts: Vec<&str> = response
            .choice
            .iter()
            .filter_map(|c| match c {
                AssistantContent::Text(t) => Some(t.text.as_str()),
                _ => None,
            })
            .collect();

        if texts.is_empty() {
            return Err(Error::runtime(
                "LLM response contained no text content",
                "rig",
                false,
            ));
        }

        Ok(texts.join("\n"))
    }

    /// Parse a JSON entity array from LLM text output.
    ///
    /// Handles multiple formats:
    /// - Raw JSON array: `[{...}, ...]`
    /// - Markdown-fenced: `` ```json\n[...]\n``` ``
    /// - Single object: `{...}` (wrapped in array)
    /// - Empty / "no entities" / "none": returns empty vec
    pub fn parse_entities(text: &str) -> Result<Vec<Value>, Error> {
        let trimmed = text.trim();

        // Handle empty or "no entities" responses.
        if trimmed.is_empty()
            || trimmed.eq_ignore_ascii_case("none")
            || trimmed.eq_ignore_ascii_case("no entities")
            || trimmed == "[]"
        {
            return Ok(Vec::new());
        }

        // Try to extract JSON from markdown fences.
        let json_str = extract_fenced_json(trimmed).unwrap_or(trimmed);

        // Try parsing as array.
        if let Ok(Value::Array(arr)) = serde_json::from_str(json_str) {
            return Ok(arr);
        }

        // Try parsing as single object.
        if let Ok(obj @ Value::Object(_)) = serde_json::from_str(json_str) {
            return Ok(vec![obj]);
        }

        // Try to find embedded JSON array in the text.
        if let Some(start) = trimmed.find('[') {
            if let Some(end) = trimmed.rfind(']') {
                if start < end {
                    let substr = &trimmed[start..=end];
                    if let Ok(Value::Array(arr)) = serde_json::from_str(substr) {
                        return Ok(arr);
                    }
                }
            }
        }

        Err(Error::runtime(
            format!("Failed to parse LLM response as JSON entities: {}", truncate(trimmed, 200)),
            "rig",
            false,
        ))
    }
}

/// Parse raw JSON dicts from an LLM backend into [`Entity`] values.
///
/// Moved from the former `parse.rs` free function `parse_llm_entities`.
pub struct EntityParser;

impl EntityParser {
    /// Parse raw JSON dicts into [`Entity`] values.
    ///
    /// Expected dict keys: `category`, `entity_type`, `value`, `confidence`,
    /// and optionally `start_offset` / `end_offset`.
    pub fn parse(raw: &[Value]) -> Result<Vec<Entity>, Error> {
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
                .ok_or_else(|| {
                    Error::validation("Missing 'entity_type'".to_string(), "llm-parse")
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
                .ok_or_else(|| Error::validation("Missing 'value'".to_string(), "llm-parse"))?;

            let confidence = obj
                .get("confidence")
                .and_then(Value::as_f64)
                .ok_or_else(|| {
                    Error::validation("Missing 'confidence'".to_string(), "llm-parse")
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

/// Extract JSON content from markdown fences.
fn extract_fenced_json(text: &str) -> Option<&str> {
    // Look for ```json ... ``` or ``` ... ```
    let start_marker = if let Some(pos) = text.find("```json") {
        pos + "```json".len()
    } else if let Some(pos) = text.find("```") {
        pos + "```".len()
    } else {
        return None;
    };

    let rest = &text[start_marker..];
    // Skip optional newline after opening fence.
    let rest = rest.strip_prefix('\n').unwrap_or(rest);

    let end = rest.find("```")?;
    let content = rest[..end].trim();

    if content.is_empty() {
        None
    } else {
        Some(content)
    }
}

/// Truncate a string for display in error messages.
fn truncate(s: &str, max_len: usize) -> &str {
    if s.len() <= max_len {
        s
    } else {
        // Find a valid char boundary
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
    fn parse_entities_raw_array() {
        let text = r#"[{"category":"pii","entity_type":"email_address","value":"a@b.com","confidence":0.9,"start_offset":0,"end_offset":7}]"#;
        let result = ResponseParser::parse_entities(text).unwrap();
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn parse_entities_fenced() {
        let text = "```json\n[{\"category\":\"pii\",\"entity_type\":\"email_address\",\"value\":\"a@b.com\",\"confidence\":0.9}]\n```";
        let result = ResponseParser::parse_entities(text).unwrap();
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn parse_entities_single_object() {
        let text = r#"{"category":"pii","entity_type":"email_address","value":"a@b.com","confidence":0.9}"#;
        let result = ResponseParser::parse_entities(text).unwrap();
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn parse_entities_empty() {
        assert!(ResponseParser::parse_entities("").unwrap().is_empty());
        assert!(ResponseParser::parse_entities("none").unwrap().is_empty());
        assert!(ResponseParser::parse_entities("[]").unwrap().is_empty());
        assert!(ResponseParser::parse_entities("No entities").unwrap().is_empty());
    }

    #[test]
    fn parse_entities_embedded_array() {
        let text = "Here are the entities:\n[{\"key\":\"val\"}]\nDone.";
        let result = ResponseParser::parse_entities(text).unwrap();
        assert_eq!(result.len(), 1);
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

        let entities = EntityParser::parse(&raw).unwrap();
        assert!(entities.is_empty());
    }
}
