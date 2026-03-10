//! [`AudioSpanId`]: identifier for audio spans.

/// Identifier for an audio span within a handler.
///
/// Wraps an optional 0-based index.  For single-track handlers the
/// index is `None`; multi-track handlers can use `Some(n)` to
/// distinguish individual tracks.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AudioSpanId(pub Option<u32>);

impl AudioSpanId {
    /// Create an ID for a specific track index.
    pub fn new(index: u32) -> Self {
        Self(Some(index))
    }

    /// The 0-based track index, if present.
    pub fn index(self) -> Option<u32> {
        self.0
    }

    /// Whether this ID addresses a specific track.
    pub fn is_indexed(self) -> bool {
        self.0.is_some()
    }
}

impl From<u32> for AudioSpanId {
    fn from(index: u32) -> Self {
        Self::new(index)
    }
}
