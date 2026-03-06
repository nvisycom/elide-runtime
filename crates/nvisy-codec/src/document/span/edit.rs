//! [`SpanEdit`]: an edit to apply to a specific span.

use nvisy_core::path::ContentSource;

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
