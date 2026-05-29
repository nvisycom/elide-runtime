//! [`Located`]: a location with its production-time provenance, with
//! optional content data attached.

use nvisy_core::content::ContentSource;

/// A location tagged with the [`ContentSource`] of the handler that
/// produced it, optionally carrying the content data at that
/// location.
///
/// `Located<L>` (i.e. `Located<L, ()>`) is the bare location form
/// returned by handler [`locations()`] streams — callers can
/// attribute each location to a specific content artifact without
/// the read overhead. `Located<L, D>` is the same record with the
/// content payload attached, useful when feeding a batch into a
/// downstream service (LLM detector, OCR, validator).
///
/// The location itself remains the structural identity used as a
/// key in [`Redactions`] — the source is metadata about how the
/// location was produced, and the data (when present) is the
/// content at that location, neither part of the identity.
///
/// [`locations()`]: crate::core::Handle::locations
/// [`Redactions`]: crate::core::Redactions
#[derive(Debug, Clone, PartialEq)]
pub struct Located<L, D = ()> {
    /// The handler-level source that produced this location.
    pub source: ContentSource,
    /// The structural location within the handler's data model.
    pub location: L,
    /// The content at the location. `()` when only identity matters.
    pub data: D,
}

impl<L> Located<L, ()> {
    /// Create a new located location with no content attached.
    pub fn new(source: ContentSource, location: L) -> Self {
        Self {
            source,
            location,
            data: (),
        }
    }

    /// Attach `data` to this location, producing a `Located<L, D>`.
    pub fn with_data<D>(self, data: D) -> Located<L, D> {
        Located {
            source: self.source,
            location: self.location,
            data,
        }
    }
}
