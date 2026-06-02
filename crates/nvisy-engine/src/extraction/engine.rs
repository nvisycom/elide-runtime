//! [`ExtractionEngine`]: per-run registry of pre-built extractors,
//! plus the per-modality populator bodies the [`ExtractionPhase`]
//! dispatches into.
//!
//! Two technique slots today — [`OcrExtractor`] (for image) and
//! [`SttExtractor`] (for audio) — each `Option<Arc<_>>` because the
//! corresponding `[extractor.*]` section is itself optional.
//! Construction is eager: HTTP clients and OCR/STT engines build
//! once at startup so per-run dispatch stays cheap.
//!
//! Per-modality populators live as `&self` methods on
//! [`ExtractionEngine`] (text, tabular, image, audio). The
//! switchboard [`Self::dispatch`] routes a [`NodeMut`] to the
//! matching populator. Text/tabular are codec-native (no backend);
//! image runs OCR; audio runs STT.
//!
//! [`ExtractionPhase`]: super::ExtractionPhase
//! [`NodeMut`]: crate::core::NodeMut

use std::collections::BTreeMap;
use std::sync::Arc;

use futures::StreamExt;
use nvisy_codec::HandleModality;
use nvisy_core::Result;
use nvisy_core::content::ContentMetadata;
use nvisy_ontology::document::{Block, Document, Span};
use nvisy_ontology::modality::{
    Audio, EmbeddedDocument, Image, ImageExtraction, ImageMetadata, Tabular, TabularBlock, Text,
    TextBlock, TextContent,
};

#[cfg(feature = "image")]
use super::ocr::OcrExtractor;
#[cfg(feature = "audio")]
use super::stt::SttExtractor;
use crate::core::{NodeMut, SharedHandle};
use crate::pipeline::{Extraction, ExtractionConfig};

const TARGET: &str = "nvisy_engine::extraction";

/// Cell separator used by [`Self::populate_tabular_blocks`] to
/// concatenate per-row text. Tab is chosen because cell values
/// rarely contain it, so the resulting flat text round-trips back
/// to per-cell ranges without ambiguity.
const TABULAR_CELL_SEPARATOR: &str = "\t";

/// Registry of pre-built extractors, one per technique.
///
/// Each slot is `Option<Arc<_>>` because the corresponding
/// `[extractor.*]` section is itself optional — operators only
/// configure the techniques they need.
#[derive(Default, Clone)]
pub struct ExtractionEngine {
    /// Pre-built OCR extractor (when `[extractor.ocr]` is set).
    #[cfg(feature = "image")]
    pub ocr: Option<Arc<OcrExtractor>>,
    /// Pre-built STT extractor (when `[extractor.stt]` is set).
    #[cfg(feature = "audio")]
    pub stt: Option<Arc<SttExtractor>>,
}

impl ExtractionEngine {
    /// Build the registry once from an [`ExtractionConfig`].
    ///
    /// Each opted-in section drives one extractor construction.
    /// Construction is eager — HTTP clients and OCR/STT engines
    /// build here so per-run dispatch stays cheap.
    ///
    /// # Errors
    ///
    /// Returns the first construction error encountered.
    pub fn from_config(cfg: &ExtractionConfig) -> Result<Self> {
        #[cfg(not(any(feature = "image", feature = "audio")))]
        let _ = cfg;
        #[cfg(feature = "image")]
        let ocr = cfg
            .ocr
            .as_ref()
            .filter(|c| c.enabled)
            .map(|c| OcrExtractor::from_config(c.clone()).map(Arc::new))
            .transpose()?;
        #[cfg(feature = "audio")]
        let stt = cfg
            .stt
            .as_ref()
            .filter(|c| c.enabled)
            .map(|c| SttExtractor::from_config(c.clone()).map(Arc::new))
            .transpose()?;
        Ok(Self {
            #[cfg(feature = "image")]
            ocr,
            #[cfg(feature = "audio")]
            stt,
        })
    }

    /// `true` when no extractors are configured.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        #[cfg(feature = "image")]
        let ocr_empty = self.ocr.is_none();
        #[cfg(not(feature = "image"))]
        let ocr_empty = true;
        #[cfg(feature = "audio")]
        let stt_empty = self.stt.is_none();
        #[cfg(not(feature = "audio"))]
        let stt_empty = true;
        ocr_empty && stt_empty
    }

    /// Per-node dispatch: route a [`NodeMut`] to the matching
    /// per-modality populator on `self`. Called once per node by
    /// [`ExtractionPhase::apply`].
    ///
    /// [`ExtractionPhase::apply`]: super::ExtractionPhase::apply
    pub(super) async fn dispatch(
        &self,
        node: NodeMut<'_>,
        handle: &SharedHandle,
        metadata: &ContentMetadata,
        plan: &Extraction,
    ) -> Result<()> {
        match node {
            NodeMut::Text(doc) => {
                self.populate_text_blocks(doc, handle).await;
                self.populate_image_embeds(doc, handle).await;
            }
            NodeMut::Tabular(doc) => {
                self.populate_tabular_blocks(doc, handle).await;
            }
            NodeMut::Image(doc) => {
                self.populate_image_doc(doc, handle).await?;
            }
            NodeMut::Audio(doc) => {
                self.populate_audio_doc(doc, handle, metadata, plan).await?;
            }
        }
        Ok(())
    }

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
    /// outer handle.
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

        #[cfg(feature = "image")]
        {
            let Some(ocr) = self.ocr.as_ref() else {
                return;
            };

            // Run OCR once per nested image doc, passing the outer
            // handle through. Cannot pre-collect all nested docs
            // because each OCR call needs an exclusive
            // `&mut Document<Image>` from the same block list we're
            // iterating.
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

    /// Append one [`Block<Tabular>`] per row to `doc`. Each block
    /// carries the concatenated row text and one span per cell
    /// mapping the cell's substring range back to the codec's
    /// per-cell [`Tabular`] coordinates.
    ///
    /// Pure codec walk — the engine contributes no state today.
    pub(crate) async fn populate_tabular_blocks(
        &self,
        doc: &mut Document<Tabular>,
        handle: &SharedHandle,
    ) {
        let locations: Vec<_> = {
            let guard = handle.lock().await;
            guard.tabular_locations().collect().await
        };
        if locations.is_empty() {
            return;
        }

        // Group cells by row, preserving column order within each row.
        let mut rows: BTreeMap<u32, Vec<Tabular>> = BTreeMap::new();
        for located in locations {
            rows.entry(located.location.row_index)
                .or_default()
                .push(located.location);
        }
        for cells in rows.values_mut() {
            cells.sort_by_key(|c| c.column_index);
        }

        let mut blocks = Vec::with_capacity(rows.len());
        // Iterate by row in BTreeMap key order (ascending row_index).
        for cells in rows.into_values() {
            let mut text = String::new();
            let mut spans = Vec::with_capacity(cells.len());
            for (i, cell) in cells.into_iter().enumerate() {
                if i > 0 {
                    text.push_str(TABULAR_CELL_SEPARATOR);
                }
                let Some(value) = handle.lock().await.read_tabular(&cell).await else {
                    continue;
                };
                let value = value.into_inner();
                let start = text.len();
                text.push_str(&value);
                let end = text.len();
                spans.push(Span {
                    text_start: start,
                    text_end: end,
                    confidence: None,
                    source: cell,
                });
            }
            let block = Block::new(TabularBlock::Row { text }).with_spans(spans);
            blocks.push(block);
        }

        tracing::debug!(
            target: TARGET,
            rows = blocks.len(),
            "populated tabular document",
        );

        doc.blocks.extend(blocks);
    }

    /// Run image extraction over `doc` using the supplied codec
    /// handle. Today that's a single OCR pass when an extractor is
    /// configured; future techniques (e.g. layout segmentation,
    /// scene-text detection) stack here.
    #[cfg(feature = "image")]
    pub(crate) async fn populate_image_doc(
        &self,
        doc: &mut Document<Image>,
        handle: &SharedHandle,
    ) -> Result<()> {
        if let Some(ref ocr) = self.ocr {
            ocr.run_on_doc(doc, handle).await?;
        }
        Ok(())
    }

    /// No-op image extraction when the `image` cargo feature is off.
    /// The method stays on the surface so the dispatch call site
    /// doesn't have to be cfg-gated.
    #[cfg(not(feature = "image"))]
    pub(crate) async fn populate_image_doc(
        &self,
        _doc: &mut Document<Image>,
        _handle: &SharedHandle,
    ) -> Result<()> {
        Ok(())
    }

    /// Transcribe the audio reachable via `handle` into `doc`,
    /// optionally diarising when the plan requests it.
    #[cfg(feature = "audio")]
    pub(crate) async fn populate_audio_doc(
        &self,
        doc: &mut Document<Audio>,
        handle: &SharedHandle,
        metadata: &ContentMetadata,
        plan: &Extraction,
    ) -> Result<()> {
        if let Some(ref stt) = self.stt {
            stt.run(doc, handle, metadata, plan.audio.diarization)
                .await?;
        }
        Ok(())
    }

    /// No-op audio extraction when the `audio` cargo feature is off.
    #[cfg(not(feature = "audio"))]
    pub(crate) async fn populate_audio_doc(
        &self,
        _doc: &mut Document<Audio>,
        _handle: &SharedHandle,
        _metadata: &ContentMetadata,
        _plan: &Extraction,
    ) -> Result<()> {
        Ok(())
    }
}
