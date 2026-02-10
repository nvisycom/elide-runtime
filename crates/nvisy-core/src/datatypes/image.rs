use bytes::Bytes;
use serde::{Deserialize, Serialize};
use crate::data::DataItem;

/// An image extracted from a document or provided directly.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct ImageData {
    #[serde(flatten)]
    pub data: DataItem,
    #[serde(with = "crate::datatypes::blob::bytes_serde")]
    #[cfg_attr(feature = "schema", schemars(with = "Vec<u8>"))]
    pub image_data: Bytes,
    pub mime_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_number: Option<u32>,
}

impl ImageData {
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

    pub fn with_dimensions(mut self, width: u32, height: u32) -> Self {
        self.width = Some(width);
        self.height = Some(height);
        self
    }

    pub fn with_source_path(mut self, path: impl Into<String>) -> Self {
        self.source_path = Some(path.into());
        self
    }

    pub fn with_page_number(mut self, page: u32) -> Self {
        self.page_number = Some(page);
        self
    }
}
