//! [`Located`]: a location paired with its production-time provenance.

use nvisy_core::content::ContentSource;

/// A location tagged with the [`ContentSource`] of the handler that
/// produced it.
///
/// Returned by handler `locations()` streams so callers can attribute
/// each location to a specific content artifact. The location itself
/// remains the structural identity used as a key in
/// [`Redactions`] — the source is metadata about how the location
/// was produced, not part of its identity.
///
/// [`Redactions`]: crate::transform::Redactions
#[derive(Debug, Clone, PartialEq)]
pub struct Located<L> {
    /// The handler-level source that produced this location.
    pub source: ContentSource,
    /// The structural location within the handler's data model.
    pub location: L,
}

impl<L> Located<L> {
    /// Create a new located location.
    pub fn new(source: ContentSource, location: L) -> Self {
        Self { source, location }
    }

    /// Discard the source, returning the underlying location.
    pub fn into_location(self) -> L {
        self.location
    }

    /// Transform the inner location, keeping the source unchanged.
    pub fn map<T>(self, f: impl FnOnce(L) -> T) -> Located<T> {
        Located {
            source: self.source,
            location: f(self.location),
        }
    }
}

