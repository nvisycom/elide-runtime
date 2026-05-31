//! Text-modality extraction.
//!
//! Text content is already structured by the codec — no backend
//! call is needed. [`ExtractionEngine::populate_text_blocks`] walks
//! each codec `Located<Text>` and emits one [`Block<Text>`] with a
//! single source-mapped [`Span<Text>`].
//!
//! For rich sources (PDF/DOCX),
//! [`ExtractionEngine::populate_image_embeds`] follows up by
//! appending one [`TextBlock::Embed`] per image location the handle
//! exposes, then OCRs into each nested [`Document<Image>`] via
//! [`OcrExtractor::run_on_doc`] using the outer envelope's handle.
//! This is how the nested-document model surfaces a PDF's image
//! content under the same root `Document<Text>` that holds its
//! text.
//!
//! [`Block<Text>`]: nvisy_ontology::document::Block
//! [`Span<Text>`]: nvisy_ontology::document::Span
//! [`Document<Image>`]: nvisy_ontology::document::Document
//! [`OcrExtractor::run_on_doc`]: super::super::image::OcrExtractor::run_on_doc

use futures::StreamExt;
use nvisy_codec::HandleModality;
use nvisy_core::Result;
use nvisy_ontology::document::{Block, Document, Span};
use nvisy_ontology::modality::{
    EmbeddedDocument, Image, ImageExtraction, ImageMetadata, Text, TextBlock, TextContent,
};

use super::{ExtractDispatch, Extraction, ExtractionEngine, PlanSlice, TextPlan};
use crate::core::SharedHandle;
use crate::pipeline::PhaseTarget;

const TARGET: &str = "nvisy_engine::extraction::text";

impl ExtractionEngine {
    /// Append one [`Block<Text>`] per codec text location to `doc`.
    /// Each block's span maps its flat text (`0..text.len()`) back
    /// to the codec's [`Text`] coordinates so downstream detection
    /// can resolve entity offsets to source locations uniformly
    /// across modalities.
    pub(crate) async fn populate_text_blocks(
        &self,
        doc: &mut Document<Text>,
        handle: &SharedHandle,
    ) {
        let locations: Vec<_> = {
            let guard = handle.lock().await;
            guard.text_locations().collect().await
        };
        if locations.is_empty() {
            return;
        }

        let mut blocks = Vec::with_capacity(locations.len());
        for located in locations {
            let Some(data) = handle.lock().await.read_text(&located.location).await else {
                continue;
            };
            let text = data.into_inner();
            let span = Span {
                text_start: 0,
                text_end: text.len(),
                confidence: None,
                source: located.location,
            };
            let block =
                Block::new(TextBlock::Text(TextContent::Paragraph { text })).with_spans(vec![span]);
            blocks.push(block);
        }

        tracing::debug!(
            target: TARGET,
            blocks = blocks.len(),
            "populated text document",
        );

        doc.blocks.extend(blocks);
    }

    /// For rich handles (PDF/DOCX), append one [`TextBlock::Embed`]
    /// per image location reachable through the handle, then OCR
    /// each region into the nested [`Document<Image>`] using the
    /// outer envelope's handle.
    ///
    /// Embeds are appended *after* the text blocks, so today's block
    /// order is "all text, then all embedded images". Source-order
    /// interleaving (image embeds sitting at their source position
    /// inside the text flow) is a refinement for when the codec
    /// surfaces image-vs-text ordering — no consumer asserts on
    /// order today.
    ///
    /// When no OCR backend is configured, this still appends the
    /// embed placeholders (so downstream phases can recurse into
    /// them) but leaves the nested doc's blocks empty.
    pub(crate) async fn populate_image_embeds(
        &self,
        doc: &mut Document<Text>,
        handle: &SharedHandle,
    ) {
        let is_rich = {
            let guard = handle.lock().await;
            matches!(guard.modality(), HandleModality::Rich)
        };
        if !is_rich {
            return;
        }

        let locations: Vec<_> = {
            let guard = handle.lock().await;
            guard.image_locations().collect().await
        };
        if locations.is_empty() {
            return;
        }

        let source = doc.audit.source;
        let mut embeds_appended = 0usize;
        for _ in &locations {
            let nested =
                Document::<Image>::new(ImageMetadata::from(ImageExtraction::Pending), source);
            let block = Block::new(TextBlock::Embed(Box::new(EmbeddedDocument::Image(nested))));
            doc.blocks.push(block);
            embeds_appended += 1;
        }

        tracing::debug!(
            target: TARGET,
            embeds = embeds_appended,
            "appended image embed placeholders",
        );

        let Some(ocr) = self.ocr.as_ref() else {
            return;
        };

        // Run OCR once per nested image doc, passing the outer
        // handle through. Cannot pre-collect all nested docs because
        // each OCR call needs an exclusive `&mut Document<Image>`
        // from the same block list we're iterating.
        for block in doc.blocks.iter_mut() {
            let TextBlock::Embed(embed) = &mut block.kind else {
                continue;
            };
            let EmbeddedDocument::Image(ref mut nested) = **embed else {
                continue;
            };
            if let Err(e) = ocr.run_on_doc(nested, handle).await {
                tracing::warn!(
                    target: TARGET,
                    error = %e,
                    "OCR failed for nested image doc; leaving blocks empty",
                );
            }
        }
    }
}

#[async_trait::async_trait]
impl ExtractDispatch<Text> for ExtractionEngine {
    type Plan = TextPlan;

    async fn extract(&self, target: &mut PhaseTarget<'_, Text>, _plan: &TextPlan) -> Result<()> {
        self.populate_text_blocks(target.doc, target.handle).await;
        self.populate_image_embeds(target.doc, target.handle).await;
        Ok(())
    }
}

impl PlanSlice<Text> for Extraction {
    fn slice(&self) -> &TextPlan {
        &self.text
    }
}
