//! Per-document phase sequence for detection: extraction →
//! detection → deduplication. No redaction, no validation, no
//! export.

use nvisy_core::modality::{Audio, Image, Tabular, Text};
use nvisy_core::{Error, Result};

use crate::core::{DocumentTree, RunContext};
use crate::phases::deduplication::DeduplicationPhase;
use crate::phases::detection::DetectionPhase;
use crate::phases::extraction::ExtractionPhase;
use crate::pipeline::Plan;

const TARGET: &str = "nvisy_engine::pipeline::detection::document";

/// Detection-only document pipeline. Constructed once per
/// detection pass, shared across all per-document tasks via
/// `Arc`.
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
        plan: &Plan,
        tree: &mut DocumentTree<Text>,
    ) -> Result<()> {
        check_cancelled(ctx)?;
        self.extraction.apply_text(ctx, plan, tree).await?;
        check_cancelled(ctx)?;
        self.detection.apply_text(ctx, plan, tree).await?;
        check_cancelled(ctx)?;
        self.deduplication.apply_text(ctx, plan, tree).await?;
        check_cancelled(ctx)
    }

    pub(super) async fn run_tabular(
        &self,
        ctx: &RunContext,
        plan: &Plan,
        tree: &mut DocumentTree<Tabular>,
    ) -> Result<()> {
        check_cancelled(ctx)?;
        self.extraction.apply_tabular(ctx, plan, tree).await?;
        check_cancelled(ctx)?;
        self.detection.apply_tabular(ctx, plan, tree).await?;
        check_cancelled(ctx)?;
        self.deduplication.apply_tabular(ctx, plan, tree).await?;
        check_cancelled(ctx)
    }

    pub(super) async fn run_image(
        &self,
        ctx: &RunContext,
        plan: &Plan,
        tree: &mut DocumentTree<Image>,
    ) -> Result<()> {
        check_cancelled(ctx)?;
        self.extraction.apply_image(ctx, plan, tree).await?;
        check_cancelled(ctx)?;
        self.detection.apply_image(ctx, plan, tree).await?;
        check_cancelled(ctx)?;
        self.deduplication.apply_image(ctx, plan, tree).await?;
        check_cancelled(ctx)
    }

    pub(super) async fn run_audio(
        &self,
        ctx: &RunContext,
        plan: &Plan,
        tree: &mut DocumentTree<Audio>,
    ) -> Result<()> {
        check_cancelled(ctx)?;
        self.extraction.apply_audio(ctx, plan, tree).await?;
        check_cancelled(ctx)?;
        self.detection.apply_audio(ctx, plan, tree).await?;
        check_cancelled(ctx)?;
        self.deduplication.apply_audio(ctx, plan, tree).await?;
        check_cancelled(ctx)
    }
}

fn check_cancelled(ctx: &RunContext) -> Result<()> {
    if ctx.is_cancelled() {
        return Err(Error::cancellation("detection cancelled", TARGET));
    }
    Ok(())
}
