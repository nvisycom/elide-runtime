//! [`DocumentPipeline`]: the per-document phase sequence.
//!
//! Holds one concrete instance of each phase. Phase order is a
//! type-level fact (the field order); adding or removing a phase is
//! a struct edit, not a Vec push.
//!
//! Each typed `run_*` method drives the per-modality `apply_*` entry
//! points on every phase in order. The orchestrator picks the right
//! method based on the [`AnyTree`] variant it's dispatching.
//!
//! [`AnyTree`]: crate::core::AnyTree

use nvisy_core::modality::{Audio, Image, Tabular, Text};
use nvisy_core::{Error, Result};

use super::engine::EngineInput;
use crate::core::{DocumentTree, RunContext};
use crate::phases::deduplication::DeduplicationPhase;
use crate::phases::detection::DetectionPhase;
use crate::phases::extraction::ExtractionPhase;
use crate::phases::redaction::phase::RedactionPhase;
use crate::phases::validation::ValidationPhase;

const TARGET: &str = "nvisy_engine::pipeline::document_pipeline";

/// Per-document phase sequence: extraction → detection → dedup →
/// (redaction → validation)?. Each field is the concrete phase
/// implementation; missing optional phases are `None`.
pub(crate) struct DocumentPipeline {
    extraction: ExtractionPhase,
    detection: DetectionPhase,
    deduplication: DeduplicationPhase,
    /// Skipped on dry-run.
    redaction: Option<RedactionPhase>,
    /// Skipped on dry-run.
    validation: Option<ValidationPhase>,
}

impl DocumentPipeline {
    /// Assemble the pipeline from the per-run context.
    pub(crate) fn from_context(ctx: &RunContext) -> Self {
        let (redaction, validation) = if ctx.dry_run() {
            (None, None)
        } else {
            (
                Some(RedactionPhase::new(
                    ctx.redaction_config().clone(),
                    ctx.redaction_registries().clone(),
                )),
                Some(ValidationPhase::new()),
            )
        };
        Self {
            extraction: ExtractionPhase::new(ctx.extraction_engine().clone()),
            detection: DetectionPhase::new(ctx.recognizer_registry().clone()),
            deduplication: DeduplicationPhase::new(),
            redaction,
            validation,
        }
    }

    pub(crate) async fn run_text(
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
        if let Some(ref r) = self.redaction {
            check_cancelled(ctx)?;
            r.apply_text(ctx, input, tree).await?;
        }
        if let Some(ref v) = self.validation {
            check_cancelled(ctx)?;
            v.apply_text(ctx, input, tree).await?;
        }
        check_cancelled(ctx)
    }

    pub(crate) async fn run_tabular(
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
        if let Some(ref r) = self.redaction {
            check_cancelled(ctx)?;
            r.apply_tabular(ctx, input, tree).await?;
        }
        if let Some(ref v) = self.validation {
            check_cancelled(ctx)?;
            v.apply_tabular(ctx, input, tree).await?;
        }
        check_cancelled(ctx)
    }

    pub(crate) async fn run_image(
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
        if let Some(ref r) = self.redaction {
            check_cancelled(ctx)?;
            r.apply_image(ctx, input, tree).await?;
        }
        if let Some(ref v) = self.validation {
            check_cancelled(ctx)?;
            v.apply_image(ctx, input, tree).await?;
        }
        check_cancelled(ctx)
    }

    pub(crate) async fn run_audio(
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
        if let Some(ref r) = self.redaction {
            check_cancelled(ctx)?;
            r.apply_audio(ctx, input, tree).await?;
        }
        if let Some(ref v) = self.validation {
            check_cancelled(ctx)?;
            v.apply_audio(ctx, input, tree).await?;
        }
        check_cancelled(ctx)
    }
}

fn check_cancelled(ctx: &RunContext) -> Result<()> {
    if ctx.is_cancelled() {
        return Err(Error::cancellation("run cancelled", TARGET));
    }
    Ok(())
}
