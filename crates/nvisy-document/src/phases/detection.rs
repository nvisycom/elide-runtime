//! [`DetectionPhase`]: per-modality recognition driver.
//!
//! Walks each [`Document<M>`]'s blocks, runs the matching recognizers
//! through [`RecognizerRegistry`], filters by the plan's entity-kind
//! allowlist, and lifts block-local offsets back to absolute modality
//! coordinates. For image trees, additionally walks every image chunk
//! and runs image-side recognizers against the raw bytes.
//!
//! [`Document<M>`]: crate::document::Document
//! [`RecognizerRegistry`]: nvisy_toolkit::detection::RecognizerRegistry

use nvisy_core::Result;
use nvisy_core::entity::Entity;
use nvisy_core::modality::{
    Audio, AudioLocation, Image, ImageLocation, Overlap, Tabular, TabularLocation, Text, TextData,
    TextLocation,
};
use nvisy_core::recognition::RecognizerInput;
use nvisy_toolkit::detection::RecognizerRegistry;
use tracing::Instrument;

use crate::core::{DocumentTree, RunContext};
use crate::document::{Document, Span};
use crate::modality::{DocumentModality, ModalityBlock};
use crate::pipeline::{Detection, EngineInput};

const TARGET: &str = "nvisy_document::detection";

/// Detection phase: runs every registered recognizer over each
/// document's blocks and writes [`EntityRecord`]s to `doc.audit`.
///
/// Holds a [`RecognizerRegistry`] by value — the registry's
/// recognizer lists keep the underlying recognizers shared via `Arc`
/// inside, without an outer wrap.
///
/// [`EntityRecord`]: crate::provenance::EntityRecord
pub struct DetectionPhase {
    registry: RecognizerRegistry,
}

impl DetectionPhase {
    /// Build the phase from the shared recognizer registry. Called
    /// once per pipeline by the pipeline orchestrator.
    pub fn new(registry: RecognizerRegistry) -> Self {
        Self { registry }
    }

    pub(crate) async fn apply_text(
        &self,
        ctx: &RunContext,
        input: &EngineInput,
        tree: &mut DocumentTree<Text>,
    ) -> Result<()> {
        self.run_text_only(ctx, input, &mut tree.root).await
    }

    pub(crate) async fn apply_tabular(
        &self,
        ctx: &RunContext,
        input: &EngineInput,
        tree: &mut DocumentTree<Tabular>,
    ) -> Result<()> {
        self.run_text_only(ctx, input, &mut tree.root).await
    }

    pub(crate) async fn apply_audio(
        &self,
        ctx: &RunContext,
        input: &EngineInput,
        tree: &mut DocumentTree<Audio>,
    ) -> Result<()> {
        self.run_text_only(ctx, input, &mut tree.root).await
    }

    pub(crate) async fn apply_image(
        &self,
        ctx: &RunContext,
        input: &EngineInput,
        tree: &mut DocumentTree<Image>,
    ) -> Result<()> {
        let span = tracing::info_span!(target: TARGET, "phase", name = "detection.image");
        let run_id = ctx.shared().run_id;
        let cfg = &input.plan.detection;
        async move {
            detect_text_blocks(&self.registry, &mut tree.root, cfg, run_id).await?;
            detect_image_chunks(
                &self.registry,
                &mut tree.root,
                tree.handle.handler_mut(),
                cfg,
                run_id,
            )
            .await?;
            self.registry.reset().await;
            Ok(())
        }
        .instrument(span)
        .await
    }

    async fn run_text_only<M>(
        &self,
        ctx: &RunContext,
        input: &EngineInput,
        doc: &mut Document<M>,
    ) -> Result<()>
    where
        M: DocumentModality + LiftFromBlock,
        M::Location: Overlap,
    {
        let span = tracing::info_span!(target: TARGET, "phase", name = "detection.text_only");
        let run_id = ctx.shared().run_id;
        async move {
            detect_text_blocks(&self.registry, doc, &input.plan.detection, run_id).await?;
            self.registry.reset().await;
            Ok(())
        }
        .instrument(span)
        .await
    }
}

/// Shared text-side block loop. Used by every modality that exposes
/// text via [`ModalityBlock::scan_text`] (today: every modality).
async fn detect_text_blocks<M>(
    registry: &RecognizerRegistry,
    doc: &mut Document<M>,
    cfg: &Detection,
    run_id: uuid::Uuid,
) -> Result<()>
where
    M: DocumentModality + LiftFromBlock,
    M::Location: Overlap,
{
    if doc.blocks.is_empty() {
        return Ok(());
    }

    let mut lifted: Vec<Entity<M>> = Vec::new();
    let mut scanned_blocks = 0usize;

    for block in &doc.blocks {
        let Some(text) = block.kind.scan_text() else {
            continue;
        };
        if text.is_empty() {
            continue;
        }
        scanned_blocks += 1;

        let mut input = RecognizerInput::new(TextData::new(text.to_owned()));
        input.correlation_id = Some(run_id);

        let detected = registry.run_text(input).await?;
        for entity in detected {
            if !cfg.entity_kinds.is_empty() && !cfg.entity_kinds.contains(&entity.entity_kind) {
                continue;
            }
            let Some(location) =
                M::lift_from_block(&block.spans, entity.location.start, entity.location.end)
            else {
                tracing::debug!(
                    target: TARGET,
                    kind = %entity.entity_kind,
                    "dropping entity with no overlapping span",
                );
                continue;
            };
            lifted.push(Entity {
                id: entity.id,
                entity_id: entity.entity_id,
                entity_kind: entity.entity_kind,
                location,
                confidence: entity.confidence,
                trail: entity.trail,
            });
        }
    }

    tracing::debug!(
        target: TARGET,
        detected = lifted.len(),
        blocks = scanned_blocks,
        "appending text-detected entities",
    );
    doc.add_entities(lifted);
    Ok(())
}

/// Walk every image chunk reachable through the handle, run the
/// registry's image recognizers against the raw bytes, and append
/// resulting entities to the document's audit.
#[cfg(feature = "image")]
async fn detect_image_chunks(
    registry: &RecognizerRegistry,
    doc: &mut Document<Image>,
    handle: &mut dyn nvisy_codec::core::IndexedHandle<Image>,
    cfg: &Detection,
    run_id: uuid::Uuid,
) -> Result<()> {
    if registry.image.is_empty() {
        return Ok(());
    }

    let mut detected_total = 0usize;
    while let Some(chunk) = handle.next_chunk().await? {
        let mut input = RecognizerInput::new(chunk.data);
        input.correlation_id = Some(run_id);

        let detected = registry.run_image(input).await?;
        let filtered: Vec<Entity<Image>> = detected
            .into_iter()
            .filter(|e| cfg.entity_kinds.is_empty() || cfg.entity_kinds.contains(&e.entity_kind))
            .collect();
        detected_total += filtered.len();
        doc.add_entities(filtered);
    }

    tracing::debug!(
        target: TARGET,
        detected = detected_total,
        "appending image-detected entities",
    );
    Ok(())
}

#[cfg(not(feature = "image"))]
async fn detect_image_chunks(
    _registry: &RecognizerRegistry,
    _doc: &mut Document<Image>,
    _handle: &mut dyn nvisy_codec::core::IndexedHandle<Image>,
    _cfg: &Detection,
    _run_id: uuid::Uuid,
) -> Result<()> {
    Ok(())
}

// ---- block-text ↔ modality-location lifting ----------------------

/// Map a block-text byte range to an absolute `M` location using the
/// block's spans.
pub trait LiftFromBlock: DocumentModality + Sized {
    fn lift_from_block(
        spans: &[Span<Self>],
        start: usize,
        end: usize,
    ) -> Option<<Self as nvisy_core::modality::Modality>::Location>;
}

impl LiftFromBlock for Text {
    fn lift_from_block(spans: &[Span<Self>], start: usize, end: usize) -> Option<TextLocation> {
        let span = spans.iter().find(|s| s.overlaps(start, end))?;
        let span_text_start = span.text_start;
        let source_base = span.source.start;
        let lifted_start = source_base + start.saturating_sub(span_text_start);
        let lifted_end = source_base + end.saturating_sub(span_text_start);
        Some(TextLocation::new(lifted_start, lifted_end))
    }
}

impl LiftFromBlock for Tabular {
    fn lift_from_block(spans: &[Span<Self>], start: usize, end: usize) -> Option<TabularLocation> {
        let span = spans.iter().find(|s| s.overlaps(start, end))?;
        let local_start = start.saturating_sub(span.text_start);
        let local_end = end.saturating_sub(span.text_start);
        let cell = &span.source;
        Some(TabularLocation {
            row_index: cell.row_index,
            column_index: cell.column_index,
            start_offset: Some(local_start),
            end_offset: Some(local_end),
            column_name: cell.column_name.clone(),
            sheet_name: cell.sheet_name.clone(),
        })
    }
}

impl LiftFromBlock for Image {
    fn lift_from_block(spans: &[Span<Self>], start: usize, end: usize) -> Option<ImageLocation> {
        let mut iter = spans.iter().filter(|s| s.overlaps(start, end));
        let first = iter.next()?;
        let mut bbox = first.source.bounding_box;
        let image_id = first.source.image_id;
        let page_number = first.source.page_number;
        for s in iter {
            bbox = bbox.union(&s.source.bounding_box);
        }
        Some(ImageLocation {
            bounding_box: bbox,
            polygon: None,
            image_id,
            page_number,
        })
    }
}

impl LiftFromBlock for Audio {
    fn lift_from_block(spans: &[Span<Self>], start: usize, end: usize) -> Option<AudioLocation> {
        let mut iter = spans.iter().filter(|s| s.overlaps(start, end));
        let first = iter.next()?;
        let mut time_span = first.source.time_span;
        let speaker_id = first.source.speaker_id.clone();
        let audio_id = first.source.audio_id;
        for s in iter {
            time_span = time_span.union(&s.source.time_span);
        }
        Some(AudioLocation {
            time_span,
            speaker_id,
            audio_id,
        })
    }
}
