//! [`Span`]: a located piece of content with its production-time
//! provenance.

use nvisy_core::content::ContentSource;

use super::Located;

/// A located piece of content paired with its data and provenance.
///
/// Useful when a caller wants to enumerate all of a handler's
/// locations *with* the content attached — typically when feeding a
/// batch into a downstream service (LLM detector, OCR, validator).
///
/// Not used on handler trait signatures: handlers expose only cheap
/// identity via [`locations`] plus on-demand [`read`]. Construct a
/// `Span` by walking the locations stream and calling `read`.
///
/// [`locations`]: crate::handler::TextHandler::locations
/// [`read`]: crate::handler::TextHandler::read
#[derive(Debug, Clone, PartialEq)]
pub struct Span<L, D> {
    /// The handler-level source that produced this span.
    pub source: ContentSource,
    /// The structural location within the handler's data model.
    pub location: L,
    /// The content at the location.
    pub data: D,
}

impl<L, D> Span<L, D> {
    /// Create a new span from its components.
    pub fn new(source: ContentSource, location: L, data: D) -> Self {
        Self {
            source,
            location,
            data,
        }
    }

    /// Construct from a [`Located<L>`] by attaching `data`.
    pub fn from_located(located: Located<L>, data: D) -> Self {
        Self {
            source: located.source,
            location: located.location,
            data,
        }
    }

    /// Transform the data, keeping source and location unchanged.
    pub fn map<T>(self, f: impl FnOnce(D) -> T) -> Span<L, T> {
        Span {
            source: self.source,
            location: self.location,
            data: f(self.data),
        }
    }
}
