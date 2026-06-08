//! Per-document phase sequence for detection: extraction →
//! detection → deduplication. No redaction, no validation, no
//! export.
//!
//! Reuses the same `*Phase` structs as the legacy unified
//! pipeline — the split is at the orchestration layer, not the
//! phase layer. Phases don't know what mode they're running in.

use nvisy_core::modality::{Audio, Image, Tabular, Text};
use nvisy_core::{Error, Result};

use crate::core::{DocumentTree, RunContext};
use crate::phases::deduplication::DeduplicationPhase;
use crate::phases::detection::DetectionPhase;
use crate::phases::extraction::ExtractionPhase;
use crate::pipeline::engine::EngineInput;

const TARGET: &str = "nvisy_document::pipeline::detection::document";

/// Detection-only document pipeline.
///
/// Constructed once per detection pass, shared across all
/// per-document tasks via `Arc`.
pub(super) struct DetectionDocumentPipeline {
    extraction: ExtractionPhase,
    detection: DetectionPhase,
    deduplication: DeduplicationPhase,
}

impl DetectionDocumentPipeline {
    pub(super) fn from_context(ctx: &RunContext) -> Self {
        Self {
            extraction: ExtractionPhase::new(ctx.extraction_engine().clone()),
            detection: DetectionPhase::new(ctx.recognizer_registry().clone()),
            deduplication: DeduplicationPhase::new(),
        }
    }

    pub(super) async fn run_text(
        &self,
        ctx: &RunContext,
        input: &EngineInput,
        tree: &mut DocumentTree<Text>,
    ) -> Result<()> {
        check_cancelled(ctx)?;
        self.extraction.apply_text(ctx, input, tree).await?;
        check_cancelled(ctx)?;
        self.detection.apply_text(ctx, input, tree).await?;
        check_cancelled(ctx)?;
        self.deduplication.apply_text(ctx, input, tree).await?;
        check_cancelled(ctx)
    }

    pub(super) async fn run_tabular(
        &self,
        ctx: &RunContext,
        input: &EngineInput,
        tree: &mut DocumentTree<Tabular>,
    ) -> Result<()> {
        check_cancelled(ctx)?;
        self.extraction.apply_tabular(ctx, input, tree).await?;
        check_cancelled(ctx)?;
        self.detection.apply_tabular(ctx, input, tree).await?;
        check_cancelled(ctx)?;
        self.deduplication.apply_tabular(ctx, input, tree).await?;
        check_cancelled(ctx)
    }

    pub(super) async fn run_image(
        &self,
        ctx: &RunContext,
        input: &EngineInput,
        tree: &mut DocumentTree<Image>,
    ) -> Result<()> {
        check_cancelled(ctx)?;
        self.extraction.apply_image(ctx, input, tree).await?;
        check_cancelled(ctx)?;
        self.detection.apply_image(ctx, input, tree).await?;
        check_cancelled(ctx)?;
        self.deduplication.apply_image(ctx, input, tree).await?;
        check_cancelled(ctx)
    }

    pub(super) async fn run_audio(
        &self,
        ctx: &RunContext,
        input: &EngineInput,
        tree: &mut DocumentTree<Audio>,
    ) -> Result<()> {
        check_cancelled(ctx)?;
        self.extraction.apply_audio(ctx, input, tree).await?;
        check_cancelled(ctx)?;
        self.detection.apply_audio(ctx, input, tree).await?;
        check_cancelled(ctx)?;
        self.deduplication.apply_audio(ctx, input, tree).await?;
        check_cancelled(ctx)
    }
}

fn check_cancelled(ctx: &RunContext) -> Result<()> {
    if ctx.is_cancelled() {
        return Err(Error::cancellation("detection cancelled", TARGET));
    }
    Ok(())
}
