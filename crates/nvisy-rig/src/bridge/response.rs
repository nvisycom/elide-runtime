//! LLM completion response parsing.
//!
//! [`ResponseParser`] extracts text from rig-core completion responses
//! and deserializes JSON (handling markdown fences and empty responses).

use std::borrow::Cow;

use serde::de::DeserializeOwned;

use rig::completion::{AssistantContent, CompletionResponse};

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
    use serde_json::Value;

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

}
