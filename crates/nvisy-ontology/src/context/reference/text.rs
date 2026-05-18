//! Text reference data for direct comparison.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::primitive::LanguageTag;

/// A labeled text value for reference matching.
///
/// The `key` is a human/LLM-readable label describing what this value
/// represents (e.g. `"full_name"`, `"account_number"`).  The `value` is
/// the literal string used for pattern matching.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TextEntry {
    /// Human/LLM-readable label.
    pub key: String,
    /// Literal value for pattern matching.
    pub value: String,
}

/// Textual reference data as key-value pairs.
///
/// Keys describe *what* a value represents (for humans and LLMs);
/// values are the literal strings used for matching.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TextData {
    /// Key-value pairs for matching.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub entries: Vec<TextEntry>,
    /// BCP-47 language tag for locale-sensitive matching.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<String>")]
    pub language: Option<LanguageTag>,
}

impl TextData {
    /// Create text data from a list of entries.
    pub fn new(entries: Vec<TextEntry>) -> Self {
        Self {
            entries,
            language: None,
        }
    }

    /// Set the language for locale-sensitive matching.
    pub fn with_language(mut self, language: LanguageTag) -> Self {
        self.language = Some(language);
        self
    }
}

impl TextEntry {
    /// Create a labeled text entry.
    pub fn new(key: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            value: value.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_constructor_sets_no_language() {
        let data = TextData::new(vec![TextEntry::new("name", "Alice")]);
        assert_eq!(data.entries.len(), 1);
        assert!(data.language.is_none());
    }

    #[test]
    fn with_language_sets_field() {
        let tag: LanguageTag = "en-US".parse().unwrap();
        let data = TextData::new(vec![]).with_language(tag.clone());
        assert_eq!(data.language, Some(tag));
    }

    #[test]
    fn roundtrip_serde() {
        let data = TextData::new(vec![
            TextEntry::new("full_name", "Alice Smith"),
            TextEntry::new("email", "alice@example.com"),
        ])
        .with_language("en".parse().unwrap());
        let json = serde_json::to_string(&data).unwrap();
        let back: TextData = serde_json::from_str(&json).unwrap();
        assert_eq!(data.entries.len(), back.entries.len());
        assert_eq!(data.language, back.language);
    }

    #[test]
    fn roundtrip_empty() {
        let data = TextData::default();
        let json = serde_json::to_string(&data).unwrap();
        let back: TextData = serde_json::from_str(&json).unwrap();
        assert!(back.entries.is_empty());
        assert!(back.language.is_none());
    }
}
