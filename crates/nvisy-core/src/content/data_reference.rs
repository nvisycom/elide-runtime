//! Lightweight source reference for locating data within a document.

use serde::{Deserialize, Serialize};

use super::ContentSource;

/// A lightweight pointer to a specific location within a content source.
///
/// `DataReference` does **not** hold the actual data — it only records
/// *where* the data lives (a [`ContentSource`]) and an optional
/// sub-location within that source (the `mapping_id`).
///
/// # Examples
///
/// ```rust
/// use nvisy_core::content::DataReference;
/// use nvisy_core::content::ContentSource;
///
/// let source = ContentSource::new();
/// let data_ref = DataReference::new(source)
///     .with_mapping_id("line-42");
///
/// assert_eq!(data_ref.mapping_id(), Some("line-42"));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
#[derive(Serialize, Deserialize)]
pub struct DataReference {
    /// Source document this reference points into.
    source: ContentSource,

    /// Optional sub-location within the source.
    ///
    /// Examples: line numbers, byte offsets, element IDs, XPath expressions.
    #[serde(skip_serializing_if = "Option::is_none")]
    mapping_id: Option<String>,
}

impl DataReference {
    /// Create a new reference to the given source.
    pub fn new(source: ContentSource) -> Self {
        Self {
            source,
            mapping_id: None,
        }
    }

    /// Set the mapping ID (builder pattern).
    #[must_use]
    pub fn with_mapping_id(mut self, mapping_id: impl Into<String>) -> Self {
        self.mapping_id = Some(mapping_id.into());
        self
    }

    /// The content source this reference points to.
    pub fn source(&self) -> ContentSource {
        self.source
    }

    /// The sub-location within the source, if any.
    pub fn mapping_id(&self) -> Option<&str> {
        self.mapping_id.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creation() {
        let source = ContentSource::new();
        let data_ref = DataReference::new(source);

        assert_eq!(data_ref.source(), source);
        assert!(data_ref.mapping_id().is_none());
    }

    #[test]
    fn with_mapping_id() {
        let source = ContentSource::new();
        let data_ref = DataReference::new(source).with_mapping_id("line-42");

        assert_eq!(data_ref.mapping_id(), Some("line-42"));
    }

    #[test]
    fn serialization_roundtrip() {
        let source = ContentSource::new();
        let data_ref = DataReference::new(source).with_mapping_id("test-mapping");

        let json = serde_json::to_string(&data_ref).unwrap();
        let deserialized: DataReference = serde_json::from_str(&json).unwrap();

        assert_eq!(data_ref, deserialized);
    }
}
