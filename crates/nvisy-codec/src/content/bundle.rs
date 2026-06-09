//! [`Content`]: data bytes optionally paired with a caller-supplied
//! [`ContentDescriptor`].

use std::path::Path;

use derive_more::{AsRef, Deref};
use nvisy_core::Result;
use serde::{Deserialize, Serialize};

use super::{ContentData, ContentDescriptor, ContentSource};

/// Upload-shape carrier: raw bytes plus the caller's descriptive
/// metadata.
///
/// [`ContentData`] holds the bytes and source identity.
/// [`ContentDescriptor`] holds filename, MIME hint, and extras when
/// the caller has them. The descriptor is optional because some
/// import paths (raw byte uploads, generated content) have nothing
/// to attach.
///
/// After `Registry::register_content` consumes a `Content`, the
/// stored shape is a `ContentRecord` (descriptor + byte-derived
/// digest), which is what registry reads return.
#[derive(Debug, Clone, PartialEq)]
#[derive(AsRef, Deref, Serialize, Deserialize)]
pub struct Content {
    /// Raw content bytes.
    #[deref]
    #[as_ref]
    data: ContentData,
    /// Caller-supplied descriptive metadata.
    descriptor: Option<ContentDescriptor>,
}

impl From<ContentData> for Content {
    fn from(data: ContentData) -> Self {
        Self::new(data)
    }
}

impl Content {
    /// Create content from data without a descriptor.
    pub fn new(data: ContentData) -> Self {
        Self {
            data,
            descriptor: None,
        }
    }

    /// Create content with a caller-supplied descriptor.
    pub fn with_descriptor(data: ContentData, descriptor: ContentDescriptor) -> Self {
        Self {
            data,
            descriptor: Some(descriptor),
        }
    }

    /// Returns the raw content data.
    pub fn data(&self) -> &ContentData {
        &self.data
    }

    /// Returns the caller-supplied descriptor, if present.
    pub fn descriptor(&self) -> Option<&ContentDescriptor> {
        self.descriptor.as_ref()
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

    /// Caller-supplied MIME type, if any. Detected MIME isn't
    /// available pre-registration (the registry computes it).
    pub fn content_type(&self) -> Option<&str> {
        self.descriptor
            .as_ref()
            .and_then(|d| d.content_type.as_deref())
    }

    /// Original filename from the descriptor.
    pub fn filename(&self) -> Option<&Path> {
        self.descriptor.as_ref().and_then(|d| d.filename.as_deref())
    }

    /// File extension from the descriptor's source path.
    pub fn file_extension(&self) -> Option<&str> {
        self.descriptor.as_ref().and_then(|d| d.file_extension())
    }

    /// Consume and return both data and descriptor.
    pub fn into_parts(self) -> (ContentData, Option<ContentDescriptor>) {
        (self.data, self.descriptor)
    }
}
