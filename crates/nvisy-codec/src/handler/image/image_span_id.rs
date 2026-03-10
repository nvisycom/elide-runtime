//! [`ImageSpanId`]: identifier for image spans.

/// Identifier for an image span within a handler.
///
/// Wraps an optional 0-based index.  For single-image handlers the
/// index is `None`; multi-image handlers can use `Some(n)` to
/// distinguish individual images.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ImageSpanId(pub Option<u32>);
