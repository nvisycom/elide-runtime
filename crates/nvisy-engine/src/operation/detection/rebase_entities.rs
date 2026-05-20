//! [`RebaseEntities`]: extension trait that shifts per-span byte
//! offsets on every text-located entity into document-relative offsets.
//!
//! Detection backends (NER agent, pattern engine) receive a span's
//! text in isolation, so the entities they emit carry offsets relative
//! to the span (`0..span.len()`). The pipeline stores entities at
//! document-relative offsets, so we shift before appending.

use nvisy_codec::Span;
use nvisy_codec::handler::TextData;
use nvisy_ontology::entity::{Entities, Location, TextLocation};

/// Extension trait on [`Entities`] for the per-span → document-relative
/// offset shift detection ops apply to backend output.
pub(crate) trait RebaseEntities {
    /// Translate per-span byte offsets on every text-located entity
    /// into document-relative offsets by adding
    /// `span.location.start_offset`.
    ///
    /// Non-text locations pass through unchanged: detection backends
    /// only emit text entities today, but the helper is total so
    /// future image or audio backends compose cleanly.
    fn rebase_offsets(self, span: &Span<TextLocation, TextData>) -> Self;
}

impl RebaseEntities for Entities {
    fn rebase_offsets(self, span: &Span<TextLocation, TextData>) -> Self {
        let shift = span.location.start_offset;
        self.into_iter()
            .map(|mut entity| {
                if let Location::Text(ref mut loc) = entity.location {
                    loc.start_offset += shift;
                    loc.end_offset += shift;
                }
                entity
            })
            .collect()
    }
}
