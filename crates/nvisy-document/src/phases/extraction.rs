//! [`ExtractionPhase`]: Document-walking glue around the toolkit-side
//! [`ExtractorRegistry`].
//!
//! The engine knows nothing about documents — it just holds typed
//! slots of pre-built per-modality [`Extractor`] implementations. This
//! phase is the bridge: it walks each [`Document<M>`] in the
//! [`DocumentTree`], pulls bytes through the codec handle, calls the
//! engine's slot for the modality of the node, converts the
//! extractor-shaped output into per-modality [`Block<M>`] values, and
//! stamps the matching [`Extraction`] provenance value into the
//! document's metadata.
//!
//! Recursion into [`TextBlock::Embed`] children is handled here by
//! visiting the root then iterating nested embedded documents; the
//! engine has no awareness of nesting.
//!
//! [`Block<M>`]: crate::document::Block
//! [`Document<M>`]: crate::document::Document
//! [`DocumentTree`]: crate::core::DocumentTree
//! [`Extraction`]: nvisy_core::modality::ModalityExtraction::Extraction
//! [`Extractor`]: nvisy_toolkit::extraction::Extractor
//! [`ExtractorRegistry`]: nvisy_toolkit::extraction::ExtractorRegistry
//! [`TextBlock::Embed`]: crate::modality::TextBlock::Embed

use std::collections::BTreeMap;

use futures::StreamExt;
use nvisy_codec::HandleModality;
use nvisy_codec::core::Located;
use nvisy_codec::handler::ImageData as CodecImageData;
use nvisy_core::Result;
use nvisy_core::content::ContentMetadata;
use nvisy_core::modality::{Audio, Image, ImageExtraction, Tabular, Text};
#[cfg(feature = "audio")]
use nvisy_core::recognition::AudioData;
#[cfg(feature = "image")]
use nvisy_core::recognition::ImageData;
#[cfg(any(feature = "image", feature = "audio"))]
use nvisy_core::recognition::RecognizerInput;
use nvisy_toolkit::extraction::{Extractor, ExtractorRegistry};
use tracing::Instrument;

use crate::core::{DocumentTree, NodeMut, RunContext, SharedHandle};
use crate::document::{Block, Document, Span};
use crate::modality::{
    EmbeddedDocument, ImageBlock, ImageMetadata, TabularBlock, TextBlock, TextContent,
};
use crate::pipeline::{EngineInput, Extraction};

const TARGET: &str = "nvisy_engine::extraction";

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

    /// Walk the tree and run the right extractor against each node.
    /// Visits the root first, then iterates nested embedded documents.
    pub(crate) async fn apply(
        &self,
        _ctx: &RunContext,
        input: &EngineInput,
        tree: &mut DocumentTree,
    ) -> Result<()> {
        let span = tracing::info_span!(target: TARGET, "phase", name = "extraction");
        let handle = tree.handle.clone();
        let metadata = tree.metadata.clone();
        async move {
            dispatch(
                &self.engine,
                tree.root_mut(),
                &handle,
                &metadata,
                &input.plan.extraction,
            )
            .await?;
            for node in tree.embeds_mut() {
                dispatch(
                    &self.engine,
                    node,
                    &handle,
                    &metadata,
                    &input.plan.extraction,
                )
                .await?;
            }
            Ok(())
        }
        .instrument(span)
        .await
    }
}

/// Route a [`NodeMut`] to the matching per-modality populator.
async fn dispatch(
    engine: &ExtractorRegistry,
    node: NodeMut<'_>,
    handle: &SharedHandle,
    metadata: &ContentMetadata,
    plan: &Extraction,
) -> Result<()> {
    match node {
        NodeMut::Text(doc) => {
            populate_text_blocks(doc, handle).await;
            populate_image_embeds(engine, doc, handle).await;
        }
        NodeMut::Tabular(doc) => {
            populate_tabular_blocks(doc, handle).await;
        }
        NodeMut::Image(doc) => {
            populate_image_doc(engine, doc, handle).await?;
        }
        NodeMut::Audio(doc) => {
            populate_audio_doc(engine, doc, handle, metadata, plan).await?;
        }
    }
    Ok(())
}

/// Append one [`Block<Text>`] per codec text location to `doc`.
/// Each block's span maps its flat text (`0..text.len()`) back to the
/// codec's [`Text`] coordinates so downstream detection can resolve
/// entity offsets to source locations uniformly across modalities.
async fn populate_text_blocks(doc: &mut Document<Text>, handle: &SharedHandle) {
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

/// For rich handles (PDF/DOCX), append one [`TextBlock::Embed`] per
/// image location reachable through the handle, then OCR each region
/// into the nested [`Document<Image>`] using the outer handle.
///
/// When no OCR backend is configured, this still appends the embed
/// placeholders (so downstream phases can recurse into them) but
/// leaves the nested doc's blocks empty.
async fn populate_image_embeds(
    engine: &ExtractorRegistry,
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
        let nested = Document::<Image>::new(ImageMetadata::from(ImageExtraction::Pending), source);
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
        let Some(ref ocr) = engine.image else {
            return;
        };

        for block in doc.blocks.iter_mut() {
            let TextBlock::Embed(embed) = &mut block.kind else {
                continue;
            };
            let EmbeddedDocument::Image(ref mut nested) = **embed else {
                continue;
            };
            if let Err(e) = run_ocr_into(ocr.as_ref(), nested, handle).await {
                tracing::warn!(
                    target: TARGET,
                    error = %e,
                    "OCR failed for nested image doc; leaving blocks empty",
                );
            }
        }
    }
    #[cfg(not(feature = "image"))]
    let _ = engine;
}

/// Append one [`Block<Tabular>`] per row to `doc`. Each block carries
/// the concatenated row text and one span per cell mapping the cell's
/// substring range back to the codec's per-cell [`Tabular`]
/// coordinates.
async fn populate_tabular_blocks(doc: &mut Document<Tabular>, handle: &SharedHandle) {
    let locations: Vec<_> = {
        let guard = handle.lock().await;
        guard.tabular_locations().collect().await
    };
    if locations.is_empty() {
        return;
    }

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

/// Run image extraction over `doc` using the supplied codec handle.
/// Today that's a single OCR pass when an extractor is configured.
#[cfg(feature = "image")]
async fn populate_image_doc(
    engine: &ExtractorRegistry,
    doc: &mut Document<Image>,
    handle: &SharedHandle,
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
    _handle: &SharedHandle,
) -> Result<()> {
    Ok(())
}

/// Drive a single OCR extractor over every image region reachable via
/// `handle`, populating `doc` with the resulting blocks and stamping
/// the extractor's provenance on the document metadata.
#[cfg(feature = "image")]
async fn run_ocr_into(
    ocr: &dyn Extractor<Image, Output = nvisy_toolkit::extraction::registry::ImageExtractorOutput>,
    doc: &mut Document<Image>,
    handle: &SharedHandle,
) -> Result<()> {
    doc.meta.extraction = ocr.extraction();

    let inputs = collect_image_inputs(handle).await;
    if inputs.is_empty() {
        return Ok(());
    }

    tracing::debug!(
        target: TARGET,
        regions = inputs.len(),
        "running OCR extraction",
    );

    for image_input in inputs {
        let png = match image_input.data.encode_png() {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!(target: TARGET, error = %e, "skipping image: PNG encode failed");
                continue;
            }
        };
        let dims = image_input.data.dimensions();
        let input = RecognizerInput::new(ImageData::new(png, dims));
        let outputs = ocr.extract(&input).await?;
        for output in outputs {
            doc.blocks.push(ocr_output_to_block(output));
        }
    }
    Ok(())
}

/// Convert a backend-shaped [`OcrOutput`] to a document-shaped
/// [`Block<Image>`].
#[cfg(feature = "image")]
fn ocr_output_to_block(output: nvisy_ocr::core::OcrOutput) -> Block<Image> {
    use nvisy_ocr::core::OcrBlockKind;
    let kind = match output.kind {
        OcrBlockKind::Text { region, text } => ImageBlock::Text { region, text },
        OcrBlockKind::Heading { region, text } => ImageBlock::Heading { region, text },
        OcrBlockKind::Table { region, text } => ImageBlock::Table { region, text },
        // Forward-compat: `OcrBlockKind` is `#[non_exhaustive]`.
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

#[cfg(feature = "image")]
async fn collect_image_inputs(handle: &SharedHandle) -> Vec<Located<Image, CodecImageData>> {
    let guard = handle.lock().await;
    let locations: Vec<Located<Image>> = guard.image_locations().collect().await;
    drop(guard);
    let mut out = Vec::with_capacity(locations.len());
    for located in locations {
        if let Some(data) = handle.lock().await.read_image(&located.location).await {
            out.push(located.with_data(data));
        }
    }
    out
}

/// Transcribe the audio reachable via `handle` into `doc`, optionally
/// diarising when the plan requests it.
#[cfg(feature = "audio")]
async fn populate_audio_doc(
    engine: &ExtractorRegistry,
    doc: &mut Document<Audio>,
    handle: &SharedHandle,
    metadata: &ContentMetadata,
    plan: &Extraction,
) -> Result<()> {
    use nvisy_codec::DocumentHandle;
    use nvisy_core::primitive::TimeSpan;

    use crate::modality::AudioBlock;

    let Some(ref stt_arc) = engine.audio else {
        return Ok(());
    };
    // Pull bytes out of the audio handle.
    let audio_bytes = {
        let handle = handle.lock().await;
        let DocumentHandle::Audio(ref handler) = *handle else {
            return Ok(());
        };
        handler.encode()?
    };

    let filename = metadata
        .filename
        .as_deref()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| "audio.wav".to_string());

    // The plan-side diarization toggle is per-request; the engine's
    // SttExtractor is built once at startup with diarization=false.
    // Until real diarization lands (#239) the per-call toggle just
    // logs a warning; provenance reflects the engine-side flag.
    doc.meta.extraction = stt_arc.extraction();

    if plan.audio.diarization {
        tracing::warn!(target: TARGET, "diarization not yet supported, skipping");
    }

    let input = RecognizerInput::new(AudioData::new(audio_bytes.as_bytes().to_vec(), filename));
    let stt_out = stt_arc.extract(&input).await?;

    if stt_out.text.is_empty() {
        tracing::debug!(target: TARGET, "transcription returned empty text");
        return Ok(());
    }

    let time_span = TimeSpan::new(0, 0);
    doc.blocks.push(Block::new(AudioBlock::Speech {
        time_span,
        text: stt_out.text.clone(),
        speaker_id: None,
    }));

    tracing::debug!(target: TARGET, "audio transcript captured");
    Ok(())
}

#[cfg(not(feature = "audio"))]
async fn populate_audio_doc(
    _engine: &ExtractorRegistry,
    _doc: &mut Document<Audio>,
    _handle: &SharedHandle,
    _metadata: &ContentMetadata,
    _plan: &Extraction,
) -> Result<()> {
    Ok(())
}
