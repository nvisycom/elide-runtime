//! [`AudioData`]: opaque wrapper around raw audio bytes.

use bytes::Bytes;
use derive_more::{AsRef, From, Into};

/// Opaque wrapper around raw audio bytes. Mirrors [`ImageData`] and
/// [`TextData`] so the per-modality `Handle<M>` impls share a
/// consistent type boundary.
///
/// [`ImageData`]: crate::handler::ImageData
/// [`TextData`]: crate::handler::TextData
#[derive(Debug, Clone, From, Into, AsRef)]
pub struct AudioData(Bytes);

impl AudioData {
    /// Create from raw bytes.
    pub fn new(bytes: impl Into<Bytes>) -> Self {
        Self(bytes.into())
    }

    /// View the inner bytes.
    pub fn as_bytes(&self) -> &Bytes {
        &self.0
    }
}
