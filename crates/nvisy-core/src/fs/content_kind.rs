//! Content type classification for different categories of data
//!
//! This module provides the [`ContentKind`] enum for classifying content
//! into broad categories. Extension-to-kind mapping is handled by the
//! engine's format registry.

use serde::{Deserialize, Serialize};
use strum::{AsRefStr, Display, EnumIter, EnumString};

/// Content type classification for different categories of data
///
/// This enum represents high-level content categories without knowledge
/// of specific file extensions or MIME types. The engine's format registry
/// handles the mapping from extensions/MIME types to content kinds.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
#[derive(AsRefStr, Display, EnumString, EnumIter)]
#[derive(Serialize, Deserialize)]
#[strum(serialize_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum ContentKind {
    /// Plain text content
    Text,
    /// Document files (PDF, Word, etc.)
    Document,
    /// Spreadsheet files (Excel, CSV, etc.)
    Spreadsheet,
    /// Image files
    Image,
    /// Archive files (ZIP, TAR, etc.)
    Archive,
    /// Unknown or unsupported content type
    #[default]
    Unknown,
}

impl ContentKind {
    /// Check if this content kind represents text-based content
    #[must_use]
    pub fn is_text_based(&self) -> bool {
        matches!(self, Self::Text)
    }

    /// Check if this content kind represents a document
    #[must_use]
    pub fn is_document(&self) -> bool {
        matches!(self, Self::Document)
    }

    /// Check if this content kind represents a spreadsheet
    #[must_use]
    pub fn is_spreadsheet(&self) -> bool {
        matches!(self, Self::Spreadsheet)
    }

    /// Check if this content kind represents an image
    #[must_use]
    pub fn is_image(&self) -> bool {
        matches!(self, Self::Image)
    }

    /// Check if this content kind represents an archive
    #[must_use]
    pub fn is_archive(&self) -> bool {
        matches!(self, Self::Archive)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_content_kind_predicates() {
        assert!(ContentKind::Text.is_text_based());
        assert!(!ContentKind::Document.is_text_based());

        assert!(ContentKind::Document.is_document());
        assert!(!ContentKind::Text.is_document());

        assert!(ContentKind::Spreadsheet.is_spreadsheet());
        assert!(!ContentKind::Document.is_spreadsheet());

        assert!(ContentKind::Image.is_image());
        assert!(!ContentKind::Text.is_image());

        assert!(ContentKind::Archive.is_archive());
        assert!(!ContentKind::Document.is_archive());
    }

    #[test]
    fn test_content_kind_display() {
        assert_eq!(ContentKind::Text.to_string(), "text");
        assert_eq!(ContentKind::Document.to_string(), "document");
        assert_eq!(ContentKind::Spreadsheet.to_string(), "spreadsheet");
        assert_eq!(ContentKind::Image.to_string(), "image");
        assert_eq!(ContentKind::Archive.to_string(), "archive");
        assert_eq!(ContentKind::Unknown.to_string(), "unknown");
    }

    #[test]
    fn test_content_kind_as_ref() {
        assert_eq!(ContentKind::Text.as_ref(), "text");
        assert_eq!(ContentKind::Document.as_ref(), "document");
    }

    #[test]
    fn test_content_kind_from_str() {
        use std::str::FromStr;

        assert_eq!(ContentKind::from_str("text").unwrap(), ContentKind::Text);
        assert_eq!(
            ContentKind::from_str("document").unwrap(),
            ContentKind::Document
        );
        assert!(ContentKind::from_str("invalid").is_err());
    }

    #[test]
    fn test_default() {
        assert_eq!(ContentKind::default(), ContentKind::Unknown);
    }

    #[test]
    fn test_serialization() {
        let kind = ContentKind::Spreadsheet;
        let serialized = serde_json::to_string(&kind).unwrap();
        assert_eq!(serialized, "\"spreadsheet\"");

        let deserialized: ContentKind = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized, kind);
    }
}
