//! Native text extraction: walk the codec handle and produce
//! [`Block<Text>`]s with source-mapped spans.
//!
//! Unlike OCR/STT, this is not a backend call — text content is
//! already structured by the codec. Each [`Located<Text>`] yielded
//! by the handle becomes one block; the block's single
//! [`Span<Text>`] maps the entire block text back to the
//! originating codec coordinates.

use nvisy_ontology::document::{Block, Span};
use nvisy_ontology::modality::{Text, TextBlock};

use crate::envelope::DocumentEnvelope;

const TARGET: &str = "nvisy_engine::extraction::text";

/// Append one [`Block<Text>`] per codec text location to the
/// envelope's document. Each block's span maps its flat text
/// (`0..text.len()`) back to the codec's [`Text`] coordinates so
/// downstream detection can resolve entity offsets to source
/// locations uniformly across modalities.
pub(super) async fn populate_document(envelope: &mut DocumentEnvelope<Text>) {
    let located = envelope.collect_text_located().await;
    if located.is_empty() {
        return;
    }

    let mut blocks = Vec::with_capacity(located.len());
    for item in located {
        let text = item.data.into_inner();
        let span = Span {
            text_start: 0,
            text_end: text.len(),
            confidence: None,
            source: item.location,
        };
        let block = Block::new(TextBlock::Paragraph { text }).with_spans(vec![span]);
        blocks.push(block);
    }

    tracing::debug!(
        target: TARGET,
        blocks = blocks.len(),
        "populated text document",
    );

    envelope.document.blocks.extend(blocks);
}
