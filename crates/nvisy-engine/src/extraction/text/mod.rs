//! Text-modality extraction.
//!
//! Text content is already structured by the codec — no backend
//! call is needed. [`populate_document`] walks each codec
//! [`Located<Text>`] and emits one [`Block<Text>`] with a
//! single source-mapped [`Span<Text>`].

use nvisy_core::Result;
use nvisy_ontology::document::{Block, Span};
use nvisy_ontology::modality::{Text, TextBlock};

use super::{Extract, Extraction, Extractors, TextWorkflow, WorkflowSlice};
use crate::envelope::DocumentEnvelope;

const TARGET: &str = "nvisy_engine::extraction::text";

/// Append one [`Block<Text>`] per codec text location to the
/// envelope's document. Each block's span maps its flat text
/// (`0..text.len()`) back to the codec's [`Text`] coordinates so
/// downstream detection can resolve entity offsets to source
/// locations uniformly across modalities.
async fn populate_document(envelope: &mut DocumentEnvelope<Text>) {
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

#[async_trait::async_trait]
impl Extract<Text> for Extractors {
    type Workflow = TextWorkflow;

    async fn extract(
        &self,
        envelope: &mut DocumentEnvelope<Text>,
        _workflow: &TextWorkflow,
    ) -> Result<()> {
        populate_document(envelope).await;
        Ok(())
    }
}

impl WorkflowSlice<Text> for Extraction {
    fn slice(&self) -> &TextWorkflow {
        &self.text
    }
}
