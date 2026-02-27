//! Span types for content traversal and editing.

use nvisy_core::path::ContentSource;

/// A span of content tagged with its origin in the source structure.
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

impl<Id: Clone, Data: Clone> Span<Id, Data> {
    /// Clone this span into a [`SpanEdit`] with the same id and data.
    pub fn to_edit(&self) -> SpanEdit<Id, Data> {
        SpanEdit {
            source: self.source,
            id: self.id.clone(),
            data: self.data.clone(),
        }
    }
}

/// An edit to apply to a specific span.
#[derive(Debug, Clone)]
pub struct SpanEdit<Id, Data> {
    /// Content source identity and lineage.
    pub source: ContentSource,
    /// Which span to edit (must match a `Span::id`).
    pub id: Id,
    /// Replacement data for this span.
    pub data: Data,
}

impl<Id, Data> SpanEdit<Id, Data> {
    /// Create a new span edit with the given identifier and replacement data.
    pub fn new(id: Id, data: Data) -> Self {
        Self {
            source: ContentSource::default(),
            id,
            data,
        }
    }

    /// Transform the replacement data, keeping the identifier and source unchanged.
    pub fn map<T>(self, f: impl FnOnce(Data) -> T) -> SpanEdit<Id, T> {
        SpanEdit {
            source: self.source,
            id: self.id,
            data: f(self.data),
        }
    }
}

#[cfg(test)]
mod tests {
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

    #[test]
    fn span_to_edit() {
        let span = Span::new(7u32, "world".to_string());
        let edit = span.to_edit();
        assert_eq!(edit.id, 7);
        assert_eq!(edit.data, "world");
    }

    #[test]
    fn span_edit_new_sets_default_source() {
        let edit = SpanEdit::new(3u32, "replacement");
        assert_eq!(edit.id, 3);
        assert_eq!(edit.data, "replacement");
    }

    #[test]
    fn span_edit_map_transforms_data() {
        let edit = SpanEdit::new(0u32, "hello");
        let mapped = edit.map(|d| d.to_uppercase());
        assert_eq!(mapped.id, 0);
        assert_eq!(mapped.data, "HELLO");
    }
}
