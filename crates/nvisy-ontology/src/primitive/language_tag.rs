//! BCP-47 language tag type.

use derive_more::{Display, FromStr};
use serde::{Deserialize, Serialize};

/// A validated [BCP-47](https://www.rfc-editor.org/info/bcp47) language tag.
///
/// Wraps [`oxilangtag::LanguageTag`] with serde support. Use
/// `#[schemars(with = "String")]` on fields of this type for JSON Schema
/// generation.
///
/// # Examples
///
/// ```
/// use nvisy_ontology::primitive::LanguageTag;
///
/// let tag: LanguageTag = "en-US".parse().unwrap();
/// assert_eq!(tag.as_str(), "en-US");
/// assert_eq!(tag.primary_language(), "en");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Display, FromStr)]
#[derive(Serialize, Deserialize)]
#[serde(transparent)]
pub struct LanguageTag(oxilangtag::LanguageTag<String>);

impl LanguageTag {
    /// Returns the tag as a string slice.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    /// Returns the primary language subtag (e.g. `"en"` from `"en-US"`).
    pub fn primary_language(&self) -> &str {
        self.0.primary_language()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple() {
        let tag: LanguageTag = "en".parse().unwrap();
        assert_eq!(tag.as_str(), "en");
        assert_eq!(tag.primary_language(), "en");
    }

    #[test]
    fn parse_with_region() {
        let tag: LanguageTag = "en-US".parse().unwrap();
        assert_eq!(tag.as_str(), "en-US");
        assert_eq!(tag.primary_language(), "en");
    }

    #[test]
    fn parse_invalid() {
        assert!("not a valid tag!!!".parse::<LanguageTag>().is_err());
    }

    #[test]
    fn serde_roundtrip() {
        let tag: LanguageTag = "uk-UA".parse().unwrap();
        let json = serde_json::to_string(&tag).unwrap();
        assert_eq!(json, "\"uk-UA\"");
        let back: LanguageTag = serde_json::from_str(&json).unwrap();
        assert_eq!(tag, back);
    }

    #[test]
    fn display() {
        let tag: LanguageTag = "de-AT".parse().unwrap();
        assert_eq!(format!("{tag}"), "de-AT");
    }
}
