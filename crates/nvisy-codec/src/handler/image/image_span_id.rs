//! [`ImageSpanId`]: identifier for image spans.

/// Identifier for an image span within a handler.
///
/// Wraps an optional 0-based index.  For single-image handlers the
/// index is `None`; multi-image handlers can use `Some(n)` to
/// distinguish individual images.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ImageSpanId(pub Option<u32>);

impl ImageSpanId {
    /// Create an ID for a specific image index.
    pub fn new(index: u32) -> Self {
        Self(Some(index))
    }

    /// The 0-based image index, if present.
    pub fn index(self) -> Option<u32> {
        self.0
    }

    /// Whether this ID addresses a specific image.
    pub fn is_indexed(self) -> bool {
        self.0.is_some()
    }
}

impl From<u32> for ImageSpanId {
    fn from(index: u32) -> Self {
        Self::new(index)
    }
}
