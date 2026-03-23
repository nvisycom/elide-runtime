//! [`TextSpanId`]: identifier for text spans.

/// Identifier for a text span within a handler.
///
/// Wraps a 0-based positional index. Used by [`BoxedTextHandler`] and
/// [`BoxedRichHandler`] to address individual text spans after
/// re-indexing from handler-native IDs.
///
/// [`BoxedTextHandler`]: super::BoxedTextHandler
/// [`BoxedRichHandler`]: crate::handler::BoxedRichHandler
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TextSpanId(pub usize);

impl TextSpanId {
    /// Create an ID for a specific span index.
    pub fn new(index: usize) -> Self {
        Self(index)
    }

    /// The 0-based span index.
    pub fn index(self) -> usize {
        self.0
    }
}

impl From<usize> for TextSpanId {
    fn from(index: usize) -> Self {
        Self::new(index)
    }
}

impl std::fmt::Display for TextSpanId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
