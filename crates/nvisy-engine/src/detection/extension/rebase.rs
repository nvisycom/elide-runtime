//! [`Rebase`]: extension trait that shifts per-context byte offsets
//! on every text entity into document-relative offsets.
//!
//! Recognizers receive a span's text in isolation (via
//! [`DetectionContext::text`]), so the entities they emit carry
//! offsets relative to that text (`0..text.len()`). Callers
//! storing entities against a larger document shift the offsets
//! by the span's start so the entity coordinates align with the
//! document.
//!
//! [`DetectionContext::text`]: crate::DetectionContext::text

use nvisy_codec::Span;
use nvisy_codec::handler::TextData;
use nvisy_ontology::entity::Entity;
use nvisy_ontology::modality::Text;

/// Extension trait on `Vec<Entity<Text>>` for the per-context →
/// document-relative offset shift detection callers apply to
/// recognizer output.
pub trait Rebase {
    /// Translate per-context byte offsets on every text entity into
    /// document-relative offsets by adding `span.location.start_offset`.
    fn rebase_offsets(self, span: &Span<Text, TextData>) -> Self;
}

impl Rebase for Vec<Entity<Text>> {
    fn rebase_offsets(self, span: &Span<Text, TextData>) -> Self {
        let shift = span.location.start_offset;
        self.into_iter()
            .map(|mut entity| {
                entity.location.start_offset += shift;
                entity.location.end_offset += shift;
                entity
            })
            .collect()
    }
}
