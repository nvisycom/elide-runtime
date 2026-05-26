//! [`RebaseEntities`]: extension trait that shifts per-context
//! byte offsets on every text-located entity into document-relative
//! offsets.
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
use nvisy_ontology::entity::Entities;
use nvisy_ontology::modality::{AnyModality, Text};

/// Extension trait on [`Entities`] for the per-context →
/// document-relative offset shift detection callers apply to
/// recognizer output.
pub trait Rebase {
    /// Translate per-context byte offsets on every text-located
    /// entity into document-relative offsets by adding
    /// `span.location.start_offset`.
    ///
    /// Non-text locations pass through unchanged: recognizers only
    /// emit text entities today, but the helper is total so future
    /// image or audio recognizers compose cleanly.
    fn rebase_offsets(self, span: &Span<Text, TextData>) -> Self;
}

impl Rebase for Entities<AnyModality> {
    fn rebase_offsets(self, span: &Span<Text, TextData>) -> Self {
        let shift = span.location.start_offset;
        self.into_iter()
            .map(|mut entity| {
                if let AnyModality::Text(ref mut loc) = entity.location {
                    loc.start_offset += shift;
                    loc.end_offset += shift;
                }
                entity
            })
            .collect()
    }
}
