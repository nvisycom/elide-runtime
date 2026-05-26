//! Image input types for OCR backends.

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use bytes::Bytes;
use nvisy_core::media::ImageFormat;

/// Image payload passed to [`Backend::run`].
///
/// Wraps raw image bytes together with format metadata. OCR is a
/// pure data-in / data-out concern at this layer — caller-side
/// provenance (e.g. [`ContentSource`]) is tracked outside the
/// backend, alongside the call site that issued the request.
///
/// [`Backend::run`]: super::Backend::run
/// [`ContentSource`]: nvisy_core::content::ContentSource
#[derive(Debug, Clone)]
pub struct ImageInput {
    /// Raw image bytes.
    pub data: Bytes,
    /// Wire format of the image bytes.
    pub format: ImageFormat,
}

impl ImageInput {
    /// Create a new image input.
    pub fn new(data: impl Into<Bytes>, format: ImageFormat) -> Self {
        Self {
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
