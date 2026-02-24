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
