//! [`Span`]: a span of content tagged with its origin.

use nvisy_core::path::ContentSource;

/// A span of content tagged with its origin in the source structure.
///
/// Used both when reading spans from a handler and when sending
/// edits back. The `id` locates the span within the handler's data
/// model and `data` carries the content (or replacement content).
#[derive(Debug, Clone)]
pub struct Span<Id, Data> {
    /// Content source identity and lineage.
    pub source: ContentSource,
    /// Identifier locating this span within the handler's data model.
    pub id: Id,
    /// The content of this span.
    pub data: Data,
}

impl<Id, Data> Span<Id, Data> {
    /// Create a new span with the given identifier and data.
    pub fn new(id: Id, data: Data) -> Self {
        Self {
            source: ContentSource::default(),
            id,
            data,
        }
    }

    /// Set the content source on this span (builder pattern).
    pub fn with_source(mut self, source: ContentSource) -> Self {
        self.source = source;
        self
    }

    /// Transform the data, keeping the identifier and source unchanged.
    pub fn map<T>(self, f: impl FnOnce(Data) -> T) -> Span<Id, T> {
        Span {
            source: self.source,
            id: self.id,
            data: f(self.data),
        }
    }
}

#[cfg(test)]
mod tests {
    use nvisy_core::path::ContentSource;

    use super::*;

    #[test]
    fn span_new_sets_default_source() {
        let span = Span::new(42u32, "hello");
        assert_eq!(span.id, 42);
        assert_eq!(span.data, "hello");
    }

    #[test]
    fn span_with_source() {
        let source = ContentSource::new();
        let span = Span::new(0u32, "data").with_source(source);
        assert_eq!(span.source, source);
    }

    #[test]
    fn span_map_transforms_data() {
        let span = Span::new(1u32, "hello");
        let mapped = span.map(|d| d.len());
        assert_eq!(mapped.id, 1);
        assert_eq!(mapped.data, 5);
    }
}
