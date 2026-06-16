//! BCP-47 language tag type.

use derive_more::{Display, FromStr};
use serde::{Deserialize, Serialize};

use crate::Error;

/// A validated [BCP-47] language tag.
///
/// Wraps [`LanguageTag`] with serde support. Use `#[schemars(with =
/// "String")]` on fields of this type for JSON Schema generation.
///
/// # Examples
///
/// ```
/// use nvisy_core::primitive::LanguageTag;
///
/// let tag: LanguageTag = "en-US".parse().unwrap();
/// assert_eq!(tag.as_str(), "en-US");
/// assert_eq!(tag.primary_language(), "en");
/// ```
///
/// [`LanguageTag`]: oxilangtag::LanguageTag
/// [BCP-47]: https://www.rfc-editor.org/info/bcp47
#[derive(Debug, Clone, PartialEq, Eq, Hash, Display, FromStr)]
#[derive(Serialize, Deserialize)]
#[serde(transparent)]
pub struct LanguageTag(oxilangtag::LanguageTag<String>);

impl LanguageTag {
    /// Parse a BCP-47 language tag from a string.
    ///
    /// Convenience over the `FromStr` impl when the input is
    /// already a `&str` literal.
    ///
    /// # Errors
    ///
    /// Returns a validation error when `tag` is not a valid
    /// BCP-47 tag.
    pub fn new(tag: &str) -> Result<Self, Error> {
        tag.parse().map_err(|e| {
            Error::validation(
                format!("invalid BCP-47 language tag `{tag}`: {e}"),
                "nvisy-core",
            )
        })
    }

    /// Returns the tag as a string slice.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    /// Returns the primary language subtag (e.g. `"en"` from `"en-US"`).
    pub fn primary_language(&self) -> &str {
        self.0.primary_language()
    }

    /// Return `true` when `self` and `other` share the same
    /// primary language subtag.
    ///
    /// Compares only the primary subtag, so `"en"`, `"en-US"`, and
    /// `"en-GB"` all match each other; `"en"` does not match
    /// `"de"`. ASCII case-insensitive (BCP-47 tags are
    /// case-insensitive by spec).
    ///
    /// # Examples
    ///
    /// ```
    /// use nvisy_core::primitive::LanguageTag;
    ///
    /// let en = LanguageTag::new("en").unwrap();
    /// let en_us = LanguageTag::new("en-US").unwrap();
    /// let de = LanguageTag::new("de").unwrap();
    ///
    /// assert!(en.matches(&en_us));
    /// assert!(en_us.matches(&en));
    /// assert!(!en.matches(&de));
    /// ```
    #[must_use]
    pub fn matches(&self, other: &Self) -> bool {
        self.primary_language()
            .eq_ignore_ascii_case(other.primary_language())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tag(s: &str) -> LanguageTag {
        LanguageTag::new(s).expect("valid BCP-47 tag")
    }

    #[test]
    fn matches_same_primary_subtag() {
        assert!(tag("en").matches(&tag("en-US")));
        assert!(tag("en-US").matches(&tag("en")));
        assert!(tag("en-US").matches(&tag("en-GB")));
        assert!(tag("en").matches(&tag("en")));
    }

    #[test]
    fn matches_rejects_distinct_primary_subtags() {
        assert!(!tag("en").matches(&tag("de")));
        assert!(!tag("en-US").matches(&tag("de-DE")));
    }

    #[test]
    fn matches_is_case_insensitive() {
        assert!(tag("EN").matches(&tag("en-us")));
    }
}
