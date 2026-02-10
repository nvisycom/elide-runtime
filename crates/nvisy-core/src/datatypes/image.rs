//! Image data extracted from documents or provided directly.

use bytes::Bytes;
use serde::{Deserialize, Serialize};
use super::DataItem;

/// An image extracted from a document or provided directly.
///
/// Carries the raw pixel data, MIME type, optional dimensions, and
/// provenance information linking back to its source.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct ImageData {
    /// Common data-item fields (id, parent_id, metadata).
    #[serde(flatten)]
    pub data: DataItem,
    /// Raw image bytes (PNG, JPEG, etc.).
    #[serde(with = "crate::datatypes::blob::bytes_serde")]
    #[cfg_attr(feature = "schema", schemars(with = "Vec<u8>"))]
    pub image_data: Bytes,
    /// MIME type of the image (e.g. `"image/png"`).
    pub mime_type: String,
    /// Width of the image in pixels, if known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    /// Height of the image in pixels, if known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
    /// File path or URL the image was loaded from, if applicable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_path: Option<String>,
    /// 1-based page number the image was extracted from, if applicable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_number: Option<u32>,
}

impl ImageData {
    /// Create a new image from raw bytes and a MIME type.
    pub fn new(image_data: impl Into<Bytes>, mime_type: impl Into<String>) -> Self {
        Self {
            data: DataItem::new(),
            image_data: image_data.into(),
            mime_type: mime_type.into(),
            width: None,
            height: None,
            source_path: None,
            page_number: None,
        }
    }

    /// Set the pixel dimensions of the image.
    pub fn with_dimensions(mut self, width: u32, height: u32) -> Self {
        self.width = Some(width);
        self.height = Some(height);
        self
    }

    /// Record the file path or URL the image originated from.
    pub fn with_source_path(mut self, path: impl Into<String>) -> Self {
        self.source_path = Some(path.into());
        self
    }

    /// Set the page number this image was extracted from.
    pub fn with_page_number(mut self, page: u32) -> Self {
        self.page_number = Some(page);
        self
    }
}
