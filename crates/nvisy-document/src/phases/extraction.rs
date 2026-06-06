//! [`ExtractionPhase`]: pulls chunks through the codec and writes
//! per-modality [`Block<M>`] values onto each [`DocumentTree<M>`].
//!
//! Text and tabular extraction drive [`Handle::next_chunk`] — one
//! pass through the codec yields `(location, data)` pairs the phase
//! turns into blocks. Image and audio still go through the toolkit
//! extractor slots (OCR / STT) because the codec only carries raw
//! bytes for those modalities; the per-extractor output is what
//! becomes a [`Block<Image>`] or [`Block<Audio>`].
//!
//! [`Block<M>`]: crate::document::Block
//! [`DocumentTree<M>`]: crate::core::DocumentTree
//! [`Handle::next_chunk`]: nvisy_codec::core::Handle::next_chunk

use std::collections::BTreeMap;

use nvisy_codec::content::ContentMetadata;
use nvisy_core::Result;
#[cfg(any(feature = "image", feature = "audio"))]
use nvisy_core::extraction::Span as ExtractionSpan;
use nvisy_core::modality::{
    Audio, Image, ImageExtraction, ImageLocation, Tabular, TabularLocation, Text,
};
#[cfg(feature = "image")]
use nvisy_core::primitive::BoundingBox;
use nvisy_ocr::core::OcrOutput;
use nvisy_toolkit::extraction::registry::ImageExtractorOutput;
use nvisy_toolkit::extraction::{Extractor, ExtractorRegistry};
use tracing::Instrument;

use crate::core::{DocumentTree, RunContext};
use crate::document::{Block, Document, Span};
use crate::modality::{ImageBlock, TabularBlock, TextBlock, TextContent};
use crate::pipeline::{EngineInput, Extraction};

const TARGET: &str = "nvisy_document::extraction";

/// Cell separator used by [`populate_tabular_blocks`] to concatenate
/// per-row text. Tab is chosen because cell values rarely contain it,
/// so the resulting flat text round-trips back to per-cell ranges
/// without ambiguity.
const TABULAR_CELL_SEPARATOR: &str = "\t";

/// Extraction phase: pulls bytes through the codec, runs the matching
/// extractor, writes [`Block<M>`] values into each document.
///
/// Holds an [`ExtractorRegistry`] by value — the engine's per-slot
/// `Option<Arc<…>>` keeps the underlying OCR/STT services shared
/// across runs without an outer wrap.
///
/// [`Block<M>`]: crate::document::Block
/// [`ExtractorRegistry`]: nvisy_toolkit::extraction::ExtractorRegistry
pub struct ExtractionPhase {
    engine: ExtractorRegistry,
}

impl ExtractionPhase {
    /// Build the phase from the shared extraction engine. Called once
    /// per pipeline by the pipeline orchestrator.
    pub fn new(engine: ExtractorRegistry) -> Self {
        Self { engine }
    }

    /// Apply to a [`Text`] tree: walk every text chunk, append one
    /// block per chunk with a single span mapping flat block text
    /// back to the codec's [`Text`] coordinates.
    pub(crate) async fn apply_text(
        &self,
        _ctx: &RunContext,
        _input: &EngineInput,
        tree: &mut DocumentTree<Text>,
    ) -> Result<()> {
        let span = tracing::info_span!(target: TARGET, "phase", name = "extraction.text");
        async move { populate_text_blocks(&mut tree.root, tree.handle.handler_mut()).await }
            .instrument(span)
            .await
    }

    /// Apply to a [`Tabular`] tree: walk every cell chunk and group
    /// cells into one block per row, with one span per cell mapping
    /// the cell's substring back to its `(row, col)` coordinates.
    pub(crate) async fn apply_tabular(
        &self,
        _ctx: &RunContext,
        _input: &EngineInput,
        tree: &mut DocumentTree<Tabular>,
    ) -> Result<()> {
        let span = tracing::info_span!(target: TARGET, "phase", name = "extraction.tabular");
        async move { populate_tabular_blocks(&mut tree.root, tree.handle.handler_mut()).await }
            .instrument(span)
            .await
    }

    /// Apply to an [`Image`] tree: run the OCR extractor against every
    /// image chunk and write the extractor's output as
    /// [`Block<Image>`] values.
    pub(crate) async fn apply_image(
        &self,
        _ctx: &RunContext,
        _input: &EngineInput,
        tree: &mut DocumentTree<Image>,
    ) -> Result<()> {
        let span = tracing::info_span!(target: TARGET, "phase", name = "extraction.image");
        async move { populate_image_doc(&self.engine, &mut tree.root, tree.handle.handler_mut()).await }
            .instrument(span)
            .await
    }

    /// Apply to an [`Audio`] tree: pull the audio chunk's bytes, run
    /// the STT extractor, write the transcript as a [`Block<Audio>`].
    pub(crate) async fn apply_audio(
        &self,
        _ctx: &RunContext,
        input: &EngineInput,
        tree: &mut DocumentTree<Audio>,
    ) -> Result<()> {
        let span = tracing::info_span!(target: TARGET, "phase", name = "extraction.audio");
        let metadata = tree.metadata.clone();
        async move {
            populate_audio_doc(
                &self.engine,
                &mut tree.root,
                tree.handle.handler_mut(),
                &metadata,
                &input.plan.extraction,
            )
            .await
        }
        .instrument(span)
        .await
    }
}

/// Walk the codec's text chunks and append one [`Block<Text>`] per
/// chunk. Each block's single span maps `0..text.len()` back to the
/// codec's [`Text`] coordinates so downstream detection can resolve
/// entity offsets to source locations uniformly across modalities.
async fn populate_text_blocks(
    doc: &mut Document<Text>,
    handle: &mut dyn nvisy_codec::core::IndexedHandle<Text>,
) -> Result<()> {
    let mut blocks = Vec::new();
    while let Some(chunk) = handle.next_chunk().await? {
        let text = chunk.data.into_string();
        let span = Span {
            text_start: 0,
            text_end: text.len(),
            confidence: None,
            source: chunk.location,
        };
        let block =
            Block::new(TextBlock::Text(TextContent::Paragraph { text })).with_spans(vec![span]);
        blocks.push(block);
    }

    if blocks.is_empty() {
        return Ok(());
    }
    tracing::debug!(
        target: TARGET,
        blocks = blocks.len(),
        "populated text document",
    );
    doc.blocks.extend(blocks);
    Ok(())
}

/// Walk the codec's tabular chunks and group cells into one block per
/// row. Each block carries the row's concatenated text and one span
/// per cell mapping the substring range back to the codec's
/// [`Tabular`] coordinates.
async fn populate_tabular_blocks(
    doc: &mut Document<Tabular>,
    handle: &mut dyn nvisy_codec::core::IndexedHandle<Tabular>,
) -> Result<()> {
    let mut rows: BTreeMap<u32, Vec<(TabularLocation, String)>> = BTreeMap::new();
    while let Some(chunk) = handle.next_chunk().await? {
        let value = chunk.data.into_string();
        rows.entry(chunk.location.row_index)
            .or_default()
            .push((chunk.location, value));
    }
    if rows.is_empty() {
        return Ok(());
    }
    for cells in rows.values_mut() {
        cells.sort_by_key(|(loc, _)| loc.column_index);
    }

    let mut blocks = Vec::with_capacity(rows.len());
    for cells in rows.into_values() {
        let mut text = String::new();
        let mut spans = Vec::with_capacity(cells.len());
        for (i, (cell, value)) in cells.into_iter().enumerate() {
            if i > 0 {
                text.push_str(TABULAR_CELL_SEPARATOR);
            }
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
    Ok(())
}

/// Run image extraction over `doc` using the supplied codec handle.
/// Today that's a single OCR pass when an extractor is configured.
#[cfg(feature = "image")]
async fn populate_image_doc(
    engine: &ExtractorRegistry,
    doc: &mut Document<Image>,
    handle: &mut dyn nvisy_codec::core::IndexedHandle<Image>,
) -> Result<()> {
    if let Some(ref ocr) = engine.image {
        run_ocr_into(ocr.as_ref(), doc, handle).await?;
    }
    Ok(())
}

#[cfg(not(feature = "image"))]
async fn populate_image_doc(
    _engine: &ExtractorRegistry,
    _doc: &mut Document<Image>,
    _handle: &mut dyn nvisy_codec::core::IndexedHandle<Image>,
) -> Result<()> {
    Ok(())
}

/// Drive a single OCR extractor over every image chunk reachable via
/// `handle`, populating `doc` with the resulting blocks and stamping
/// the extractor's provenance on the document metadata.
#[cfg(feature = "image")]
async fn run_ocr_into(
    ocr: &dyn Extractor<Image, Output = ImageExtractorOutput>,
    doc: &mut Document<Image>,
    handle: &mut dyn nvisy_codec::core::IndexedHandle<Image>,
) -> Result<()> {
    while let Some(chunk) = handle.next_chunk().await? {
        let dims = chunk.data.dims;
        let location = ImageLocation::new(BoundingBox::new(
            0.0,
            0.0,
            f64::from(dims.width),
            f64::from(dims.height),
        ));
        let span = ExtractionSpan::new(chunk.data, location);
        let output = ocr.extract(&span).await?;
        doc.meta.extraction = output.extraction;
        for block in output.value {
            doc.blocks.push(ocr_output_to_block(block));
        }
    }
    if doc.blocks.is_empty() && matches!(doc.meta.extraction, ImageExtraction::Pending) {
        // No chunks streamed and no extractor output stamped — leave
        // the document in its `Pending` extraction state.
        tracing::debug!(target: TARGET, "no image chunks to OCR");
    }
    Ok(())
}

/// Convert a backend-shaped [`OcrOutput`] to a document-shaped
/// [`Block<Image>`].
#[cfg(feature = "image")]
fn ocr_output_to_block(output: OcrOutput) -> Block<Image> {
    use nvisy_ocr::core::OcrBlockKind;
    let kind = match output.kind {
        OcrBlockKind::Text { region, text } => ImageBlock::Text { region, text },
        OcrBlockKind::Heading { region, text } => ImageBlock::Heading { region, text },
        OcrBlockKind::Table { region, text } => ImageBlock::Table { region, text },
        _ => unreachable!("OcrBlockKind has no further variants"),
    };
    let spans: Vec<Span<Image>> = output
        .spans
        .into_iter()
        .map(|s| Span {
            text_start: s.text_start,
            text_end: s.text_end,
            source: s.source,
            confidence: Some(s.confidence),
        })
        .collect();
    Block {
        kind,
        spans,
        confidence: Some(output.confidence),
    }
}

/// Transcribe the audio reachable via `handle` into `doc`, optionally
/// diarising when the plan requests it.
#[cfg(feature = "audio")]
async fn populate_audio_doc(
    engine: &ExtractorRegistry,
    doc: &mut Document<Audio>,
    handle: &mut dyn nvisy_codec::core::IndexedHandle<Audio>,
    _metadata: &ContentMetadata,
    plan: &Extraction,
) -> Result<()> {
    use nvisy_core::modality::AudioLocation;

    use crate::modality::AudioBlock;

    let Some(ref stt_arc) = engine.audio else {
        return Ok(());
    };

    if plan.audio.diarization {
        tracing::warn!(target: TARGET, "diarization not yet supported, skipping");
    }

    while let Some(chunk) = handle.next_chunk().await? {
        let time_span = chunk.location.time_span;
        let span = ExtractionSpan::new(chunk.data, AudioLocation::new(time_span));
        let output = stt_arc.extract(&span).await?;
        doc.meta.extraction = output.extraction;
        let stt_out = output.value;

        if stt_out.text.is_empty() {
            tracing::debug!(target: TARGET, "transcription returned empty text");
            continue;
        }
        doc.blocks.push(Block::new(AudioBlock::Speech {
            time_span,
            text: stt_out.text.clone(),
            speaker_id: None,
        }));
    }

    tracing::debug!(target: TARGET, "audio extraction complete");
    Ok(())
}

#[cfg(not(feature = "audio"))]
async fn populate_audio_doc(
    _engine: &ExtractorRegistry,
    _doc: &mut Document<Audio>,
    _handle: &mut dyn nvisy_codec::core::IndexedHandle<Audio>,
    _metadata: &ContentMetadata,
    _plan: &Extraction,
) -> Result<()> {
    Ok(())
}
