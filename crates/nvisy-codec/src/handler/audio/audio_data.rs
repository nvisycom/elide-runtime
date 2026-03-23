//! [`AudioData`]: opaque wrapper around raw audio bytes.

use bytes::Bytes;
use derive_more::{AsRef, From, Into};

/// Opaque wrapper around raw audio bytes.
///
/// Mirrors [`ImageData`](crate::handler::ImageData) and
/// [`TextData`](crate::handler::TextData) for audio-bearing handlers,
/// providing a consistent type boundary at the `AudioHandler` trait
/// level.
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

    /// Consume the wrapper and return the inner `Bytes`.
    pub fn into_inner(self) -> Bytes {
        self.0
    }

    /// Length in bytes.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether the audio data is empty.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}
