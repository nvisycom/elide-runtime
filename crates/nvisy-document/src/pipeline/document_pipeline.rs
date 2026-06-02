//! [`DocumentPipeline`]: the per-document phase sequence.
//!
//! Replaces the old [`Phase<M>`] + dispatch-trait stack with a plain
//! struct holding one concrete instance of each phase. Phase order is
//! a type-level fact (the field order); adding or removing a phase is
//! a struct edit, not a Vec push.
//!
//! Each phase exposes an `apply(ctx, tree)` method that walks the
//! [`DocumentTree`] in pre-order and dispatches per [`NodeMut`]
//! variant to its own per-modality body. Phases own their engine
//! handles (extraction's `ExtractorRegistry`, detection's
//! `RecognizerRegistry`) so the orchestrator doesn't have to thread
//! them through.
//!
//! [`DocumentTree`]: crate::core::DocumentTree
//! [`NodeMut`]: crate::core::NodeMut

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
    /// Assemble the pipeline from the per-run context. Each phase
    /// clones its long-lived engine / config out of `ctx`. The
    /// clones are cheap — the engines internally hold `Arc`-wrapped
    /// shared state.
    pub(crate) fn from_context(ctx: &RunContext) -> Self {
        let (redaction, validation) = if ctx.dry_run() {
            (None, None)
        } else {
            (
                Some(RedactionPhase::new(ctx.redaction_config().clone())),
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

    /// Drive `tree` through every phase in order. Stops on the first
    /// phase error and surfaces it to the caller.
    pub(crate) async fn run(
        &self,
        ctx: &RunContext,
        input: &EngineInput,
        tree: &mut DocumentTree,
    ) -> Result<()> {
        check_cancelled(ctx)?;
        self.extraction.apply(ctx, input, tree).await?;

        check_cancelled(ctx)?;
        self.detection.apply(ctx, input, tree).await?;

        check_cancelled(ctx)?;
        self.deduplication.apply(ctx, input, tree).await?;

        if let Some(ref redaction) = self.redaction {
            check_cancelled(ctx)?;
            redaction.apply(ctx, input, tree).await?;
        }
        if let Some(ref validation) = self.validation {
            check_cancelled(ctx)?;
            validation.apply(ctx, input, tree).await?;
        }
        check_cancelled(ctx)?;
        Ok(())
    }
}

/// Cancellation guard shared by every phase.
fn check_cancelled(ctx: &RunContext) -> Result<()> {
    if ctx.is_cancelled() {
        return Err(Error::cancellation("run cancelled", TARGET));
    }
    Ok(())
}
