//! Span types for content traversal and editing.

/// A span of content tagged with its origin in the source structure.
#[derive(Debug, Clone)]
pub struct Span<Id, Data> {
    /// Identifier locating this span within the handler's data model.
    pub id: Id,
    /// The content of this span.
    pub data: Data,
}

/// An edit to apply to a specific span.
#[derive(Debug, Clone)]
pub struct SpanEdit<Id, Data> {
    /// Which span to edit (must match a `Span::id`).
    pub id: Id,
    /// Replacement data for this span.
    pub data: Data,
}
