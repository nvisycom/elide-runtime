//! [`Content`]: data bytes paired with descriptive metadata.

use std::path::Path;

use derive_more::{AsRef, Deref};
use serde::{Deserialize, Serialize};

use super::{ContentData, ContentMetadata, ContentSource};
use crate::error::Result;
use crate::media::DocumentType;

/// Complete content representation: raw bytes + metadata.
///
/// [`ContentData`] holds the bytes and source identity.
/// [`ContentMetadata`] holds MIME type, filename, and arbitrary
/// key-value pairs. Together they form a `Content`.
#[derive(Debug, Clone, PartialEq)]
#[derive(AsRef, Deref, Serialize, Deserialize)]
pub struct Content {
    /// Raw content bytes.
    #[deref]
    #[as_ref]
    data: ContentData,
    /// Descriptive metadata (MIME type, filename, etc.).
    metadata: Option<ContentMetadata>,
}

impl From<ContentData> for Content {
    fn from(data: ContentData) -> Self {
        Self::new(data)
    }
}

impl Content {
    /// Create content from data without metadata.
    pub fn new(data: ContentData) -> Self {
        Self {
            data,
            metadata: None,
        }
    }

    /// Create content with metadata.
    pub fn with_metadata(data: ContentData, metadata: ContentMetadata) -> Self {
        Self {
            data,
            metadata: Some(metadata),
        }
    }

    /// Returns the raw content data.
    pub fn data(&self) -> &ContentData {
        &self.data
    }

    /// Returns the metadata, if present.
    pub fn metadata(&self) -> Option<&ContentMetadata> {
        self.metadata.as_ref()
    }

    /// Returns the content source identifier.
    pub fn content_source(&self) -> ContentSource {
        self.data.content_source
    }

    /// Returns the raw bytes.
    pub fn as_bytes(&self) -> &[u8] {
        self.data.as_bytes()
    }

    /// Returns `true` if the content appears to be text.
    pub fn is_likely_text(&self) -> bool {
        self.data.is_likely_text()
    }

    /// Try to get the content as a string slice.
    ///
    /// # Errors
    ///
    /// Returns an error if the content is not valid UTF-8.
    pub fn as_str(&self) -> Result<&str> {
        self.data.as_str()
    }

    /// Best-available MIME type from metadata.
    pub fn content_type(&self) -> Option<&str> {
        self.metadata.as_ref().and_then(|m| m.content_type())
    }

    /// Original filename from metadata.
    pub fn filename(&self) -> Option<&Path> {
        self.metadata.as_ref().and_then(|m| m.filename.as_deref())
    }

    /// File extension from the source path in metadata.
    pub fn file_extension(&self) -> Option<&str> {
        self.metadata.as_ref().and_then(|m| m.file_extension())
    }

    /// Infer the [`DocumentType`] from metadata (MIME, filename) with
    /// fallback to magic-byte detection on the raw bytes.
    ///
    /// Delegates to [`ContentMetadata::infer_document_type`] when
    /// metadata is present, otherwise attempts magic-byte detection.
    #[must_use]
    pub fn infer_document_type(&self) -> Option<DocumentType> {
        if let Some(ref meta) = self.metadata {
            let result = meta.infer_document_type();
            if result.is_some() {
                return result;
            }
        }
        // Last resort: magic-byte detection on raw bytes.
        self.data
            .detect_mime()
            .as_deref()
            .and_then(DocumentType::from_mime)
    }

    /// Consume and return both data and metadata.
    pub fn into_parts(self) -> (ContentData, Option<ContentMetadata>) {
        (self.data, self.metadata)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::media::{ImageFormat, TextFormat};

    #[test]
    fn infer_document_type_from_metadata() {
        let data = ContentData::from("plain text");
        let metadata = ContentMetadata::new().with_content_type("text/plain");
        let content = Content::with_metadata(data, metadata);

        assert_eq!(
            content.infer_document_type(),
            Some(DocumentType::Text(TextFormat::Txt)),
        );
    }

    #[test]
    fn infer_document_type_from_magic_bytes() {
        let png = vec![
            0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48,
            0x44, 0x52,
        ];
        let content = Content::new(ContentData::from(png));
        assert_eq!(
            content.infer_document_type(),
            Some(DocumentType::Image(ImageFormat::Png)),
        );
    }

    #[test]
    fn infer_document_type_none_for_unknown() {
        let content = Content::new(ContentData::from("hello world"));
        assert_eq!(content.infer_document_type(), None);
    }
}
