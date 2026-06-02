//! [`DetectionPhase`]: Document-walking glue around
//! [`RecognizerRegistry`].
//!
//! The registry knows nothing about documents — it only takes a
//! [`RecognizerInput`] and runs every registered recognizer in parallel.
//! This phase is the bridge: it walks each [`Document<M>`] in the
//! [`DocumentTree`], builds a `RecognizerInput` per block / image location,
//! feeds them to the registry, filters the merged result by the
//! plan's entity-kind allowlist, and lifts block-local offsets back
//! to absolute modality coordinates.
//!
//! Recursion into [`TextBlock::Embed`] children is handled here by
//! visiting the root then iterating nested embedded documents; the
//! registry has no awareness of nesting.
//!
//! [`RecognizerInput`]: nvisy_core::RecognizerInput
//! [`Document<M>`]: crate::document::Document
//! [`DocumentTree`]: crate::core::DocumentTree
//! [`RecognizerRegistry`]: nvisy_toolkit::detection::RecognizerRegistry
//! [`TextBlock::Embed`]: crate::modality::TextBlock::Embed

#[cfg(feature = "image")]
use futures::StreamExt;
use nvisy_core::entity::Entity;
use nvisy_core::modality::{Audio, Image, Overlap, Tabular, Text};
#[cfg(feature = "image")]
use nvisy_core::{Error, ImageData};
use nvisy_core::{RecognizerInput, Result, TextData};
use nvisy_toolkit::detection::RecognizerRegistry;
use tracing::Instrument;

use crate::core::{DocumentTree, NodeMut, RunContext, SharedHandle};
use crate::document::{Document, Span};
use crate::modality::ModalityBlock;
use crate::pipeline::{Detection, EngineInput};

const TARGET: &str = "nvisy_engine::detection";

/// Detection phase: runs every registered recognizer over each
/// node's blocks and writes [`EntityRecord`]s to `doc.audit`.
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

    /// Walk the tree and dispatch the right recognizers per modality
    /// node. Visits the root first, then iterates nested embedded
    /// documents; each per-node body borrows the registry, handle,
    /// and detection plan directly from this scope.
    pub(crate) async fn apply(
        &self,
        ctx: &RunContext,
        input: &EngineInput,
        tree: &mut DocumentTree,
    ) -> Result<()> {
        let span = tracing::info_span!(target: TARGET, "phase", name = "detection");
        let run_id = ctx.shared().run_id;
        // Snapshot the tree-owned handle so it doesn't conflict with
        // the per-node `&mut` borrows produced by `root_mut` /
        // `embeds_mut` further down.
        let handle = tree.handle.clone();
        async move {
            dispatch(
                &self.registry,
                tree.root_mut(),
                &handle,
                &input.plan.detection,
                run_id,
            )
            .await?;
            for node in tree.embeds_mut() {
                dispatch(&self.registry, node, &handle, &input.plan.detection, run_id).await?;
            }
            Ok(())
        }
        .instrument(span)
        .await
    }
}

/// Per-node dispatch: route a [`NodeMut`] to the matching
/// per-modality detection body, using `registry` to run the
/// recognizers.
async fn dispatch(
    registry: &RecognizerRegistry,
    node: NodeMut<'_>,
    handle: &SharedHandle,
    cfg: &Detection,
    run_id: uuid::Uuid,
) -> Result<()> {
    match node {
        NodeMut::Text(doc) => detect_text_only::<Text>(registry, doc, cfg, run_id).await,
        NodeMut::Tabular(doc) => detect_text_only::<Tabular>(registry, doc, cfg, run_id).await,
        NodeMut::Audio(doc) => detect_text_only::<Audio>(registry, doc, cfg, run_id).await,
        NodeMut::Image(doc) => detect_image(registry, doc, handle, cfg, run_id).await,
    }
}

/// Run text recognizers over every block in `doc` and reset
/// per-document state. Shared by every modality whose detection is
/// purely text-based.
async fn detect_text_only<M>(
    registry: &RecognizerRegistry,
    doc: &mut Document<M>,
    cfg: &Detection,
    run_id: uuid::Uuid,
) -> Result<()>
where
    M: crate::modality::DocumentModality
        + nvisy_toolkit::redaction::Redactable
        + LiftFromBlock
        + Overlap,
{
    detect_text_blocks(registry, doc, cfg, run_id).await?;
    registry.reset().await;
    Ok(())
}

/// Image-node detection body. Runs text recognizers over each OCR'd
/// block ("runs alongside" image-side recognizers), then image
/// recognizers once per image location with raw bytes. When the
/// `image` feature is off only the text pass runs.
#[cfg(feature = "image")]
async fn detect_image(
    registry: &RecognizerRegistry,
    doc: &mut Document<Image>,
    handle: &SharedHandle,
    cfg: &Detection,
    run_id: uuid::Uuid,
) -> Result<()> {
    detect_text_blocks(registry, doc, cfg, run_id).await?;
    detect_image_locations(registry, doc, handle, cfg, run_id).await?;
    registry.reset().await;
    Ok(())
}

#[cfg(not(feature = "image"))]
async fn detect_image(
    registry: &RecognizerRegistry,
    doc: &mut Document<Image>,
    _handle: &SharedHandle,
    cfg: &Detection,
    run_id: uuid::Uuid,
) -> Result<()> {
    detect_text_only(registry, doc, cfg, run_id).await
}

/// Shared text-side block loop. Used by every modality that exposes
/// text via [`ModalityBlock::scan_text`] (today: every modality).
///
/// Recursion into [`TextBlock::Embed`] children is the caller's job;
/// this loop scans the target's *own* blocks only and skips embeds
/// via `scan_text` returning `None`.
///
/// [`TextBlock::Embed`]: crate::modality::TextBlock::Embed
async fn detect_text_blocks<M>(
    registry: &RecognizerRegistry,
    doc: &mut Document<M>,
    cfg: &Detection,
    run_id: uuid::Uuid,
) -> Result<()>
where
    M: crate::modality::DocumentModality
        + nvisy_toolkit::redaction::Redactable
        + LiftFromBlock
        + Overlap,
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

        let mut ctx = RecognizerInput::new(TextData::new(text.to_owned()));
        ctx.correlation_id = Some(run_id);

        let detected = registry.run_text(ctx).await?;
        for entity in detected {
            // Centralised entity-kind allowlist filter.
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

/// Run image recognizers once per image location reachable via the
/// target's handle, appending detections to `doc.audit`. Each call
/// sees the raw encoded image bytes plus the image's pixel
/// dimensions; recognizers emit absolute image-coord entities
/// directly (no per-block lifting).
#[cfg(feature = "image")]
async fn detect_image_locations(
    registry: &RecognizerRegistry,
    doc: &mut Document<Image>,
    handle: &SharedHandle,
    cfg: &Detection,
    run_id: uuid::Uuid,
) -> Result<()> {
    if registry.image.is_empty() {
        return Ok(());
    }

    let locations: Vec<_> = handle.lock().await.image_locations().collect().await;
    let mut detected_total = 0usize;
    for located in locations {
        let Some(image_data) = handle.lock().await.read_image(&located.location).await else {
            continue;
        };
        let dims = image_data.dimensions();
        let bytes = image_data
            .encode_png()
            .map_err(|e| Error::runtime(e.to_string(), "recognizer-registry", false))?;

        let mut ctx = RecognizerInput::new(ImageData::new(bytes, dims));
        ctx.correlation_id = Some(run_id);

        let detected = registry.run_image(ctx).await?;
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

// ---- block-text ↔ modality-location lifting ----------------------

/// Map a block-text byte range to an absolute `M` location using the
/// block's spans.
///
/// Returns `None` when no span overlaps the requested range — the
/// dispatcher discards such entities since there's no way to place
/// them in modality coordinates.
pub trait LiftFromBlock:
    crate::modality::DocumentModality + nvisy_toolkit::redaction::Redactable + Sized
{
    /// Lift block-text byte range `[start, end)` to an absolute `M`
    /// location.
    fn lift_from_block(spans: &[Span<Self>], start: usize, end: usize) -> Option<Self>;
}

impl LiftFromBlock for Text {
    /// For text the block normally has exactly one span covering
    /// `0..text.len()` whose `source: Text` carries the
    /// document-relative offsets. The entity's lifted location
    /// shifts `[start, end)` into that span's source range.
    /// Multi-span text blocks (rare today) take the first
    /// overlapping span as the anchor.
    fn lift_from_block(spans: &[Span<Self>], start: usize, end: usize) -> Option<Self> {
        let span = spans.iter().find(|s| s.overlaps(start, end))?;
        let span_text_start = span.text_start;
        let source_base = span.source.start;
        let lifted_start = source_base + start.saturating_sub(span_text_start);
        let lifted_end = source_base + end.saturating_sub(span_text_start);
        Some(Text::new(lifted_start, lifted_end))
    }
}

impl LiftFromBlock for Tabular {
    /// Tabular cells live in distinct `(row, col)` coordinates. When
    /// the entity spans a single cell, the cell's coordinates are
    /// returned with intra-cell `start_offset`/`end_offset` adjusted
    /// to the in-cell substring. Cross-cell matches return the
    /// first overlapping cell's coordinates — they're rare and
    /// downstream redaction operates per-cell anyway.
    fn lift_from_block(spans: &[Span<Self>], start: usize, end: usize) -> Option<Self> {
        let span = spans.iter().find(|s| s.overlaps(start, end))?;
        let local_start = start.saturating_sub(span.text_start);
        let local_end = end.saturating_sub(span.text_start);
        let cell = &span.source;
        Some(Tabular {
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
    /// Image entity location is the union of overlapping word-span
    /// bounding boxes, folded via [`BoundingBox::union`].
    ///
    /// [`BoundingBox::union`]: nvisy_core::primitive::BoundingBox::union
    fn lift_from_block(spans: &[Span<Self>], start: usize, end: usize) -> Option<Self> {
        let mut iter = spans.iter().filter(|s| s.overlaps(start, end));
        let first = iter.next()?;
        let mut bbox = first.source.bounding_box;
        let image_id = first.source.image_id;
        let page_number = first.source.page_number;
        for s in iter {
            bbox = bbox.union(&s.source.bounding_box);
        }
        Some(Image {
            bounding_box: bbox,
            polygon: None,
            image_id,
            page_number,
        })
    }
}

impl LiftFromBlock for Audio {
    /// Audio entity location is the union of overlapping word-span
    /// time intervals on the same speaker.
    fn lift_from_block(spans: &[Span<Self>], start: usize, end: usize) -> Option<Self> {
        let mut iter = spans.iter().filter(|s| s.overlaps(start, end));
        let first = iter.next()?;
        let mut time_span = first.source.time_span;
        let speaker_id = first.source.speaker_id.clone();
        let audio_id = first.source.audio_id;
        for s in iter {
            time_span = time_span.union(&s.source.time_span);
        }
        Some(Audio {
            time_span,
            speaker_id,
            audio_id,
        })
    }
}
