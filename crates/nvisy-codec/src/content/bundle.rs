//! [`Content`]: data bytes optionally paired with descriptive
//! metadata.

use std::path::Path;

use derive_more::{AsRef, Deref};
use nvisy_core::Result;
use serde::{Deserialize, Serialize};

use super::{ContentData, ContentMetadata, ContentSource};

/// Complete content representation: raw bytes plus optional
/// metadata.
///
/// [`ContentData`] holds the bytes and source identity.
/// [`ContentMetadata`] holds MIME type, filename, and arbitrary
/// key-value pairs when present. Metadata is optional because some
/// import paths (raw byte uploads, generated content) have nothing
/// useful to attach.
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

    /// Consume and return both data and metadata.
    pub fn into_parts(self) -> (ContentData, Option<ContentMetadata>) {
        (self.data, self.metadata)
    }
}
