//! [`AudioSpanId`]: identifier for audio spans.

/// Identifier for an audio span within a handler.
///
/// Wraps an optional 0-based index.  For single-track handlers the
/// index is `None`; multi-track handlers can use `Some(n)` to
/// distinguish individual tracks.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AudioSpanId(pub Option<u32>);
