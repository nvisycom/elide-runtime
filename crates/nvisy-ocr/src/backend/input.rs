//! Image input types for OCR backends.

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use bytes::Bytes;
use strum::{Display, EnumString, IntoStaticStr};

use nvisy_core::path::ContentSource;

/// Image format passed to a [`Backend`](super::Backend).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Display, EnumString, IntoStaticStr)]
pub enum ImageFormat {
    #[strum(serialize = "png")]
    Png,
    #[strum(serialize = "jpeg")]
    Jpeg,
}

impl ImageFormat {
    /// MIME type string for this format.
    pub fn mime_type(self) -> &'static str {
        match self {
            Self::Png => "image/png",
            Self::Jpeg => "image/jpeg",
        }
    }
}

/// Image payload passed to [`Backend::run`](super::Backend::run).
///
/// Wraps raw image bytes together with format metadata and a
/// [`ContentSource`] for provenance tracking.
#[derive(Debug, Clone)]
pub struct ImageInput {
    /// Provenance identifier for this image.
    pub source: ContentSource,
    /// Raw image bytes.
    pub data: Bytes,
    /// Wire format of the image bytes.
    pub format: ImageFormat,
}

impl ImageInput {
    /// Create a new image input with a fresh [`ContentSource`].
    pub fn new(data: impl Into<Bytes>, format: ImageFormat) -> Self {
        Self {
            source: ContentSource::new(),
            data: data.into(),
            format,
        }
    }

    /// Create a new image input with an explicit [`ContentSource`].
    pub fn with_source(source: ContentSource, data: impl Into<Bytes>, format: ImageFormat) -> Self {
        Self {
            source,
            data: data.into(),
            format,
        }
    }

    /// MIME type string for this image.
    pub fn mime_type(&self) -> &'static str {
        self.format.mime_type()
    }

    /// Size of the raw image data in bytes.
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Returns `true` if the image data is empty.
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Encode the image data as standard base64.
    pub fn to_base64(&self) -> String {
        BASE64.encode(&self.data)
    }
}
