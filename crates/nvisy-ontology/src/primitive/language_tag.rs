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
