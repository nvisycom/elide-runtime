//! Per-document phase sequence for redaction: redaction →
//! validation.

use nvisy_core::modality::{Audio, Image, Tabular, Text};
use nvisy_core::{Error, Result};

use crate::core::{DocumentTree, Plan, RunContext};
use crate::phases::redaction::phase::RedactionPhase;
use crate::phases::validation::ValidationPhase;

const TARGET: &str = "nvisy_document::pipeline::redaction::document";

pub(super) struct RedactionDocumentPipeline {
    redaction: RedactionPhase,
    validation: ValidationPhase,
}

impl RedactionDocumentPipeline {
    pub(super) fn from_context(ctx: &RunContext) -> Self {
        Self {
            redaction: RedactionPhase::new(
                ctx.redaction_config().clone(),
                ctx.redaction_registries().clone(),
            ),
            validation: ValidationPhase::new(),
        }
    }

    pub(super) async fn run_text(
        &self,
        ctx: &RunContext,
        plan: &Plan,
        tree: &mut DocumentTree<Text>,
    ) -> Result<()> {
        check_cancelled(ctx)?;
        self.redaction.apply_text(ctx, plan, tree).await?;
        check_cancelled(ctx)?;
        self.validation.apply_text(ctx, plan, tree).await?;
        check_cancelled(ctx)
    }

    pub(super) async fn run_tabular(
        &self,
        ctx: &RunContext,
        plan: &Plan,
        tree: &mut DocumentTree<Tabular>,
    ) -> Result<()> {
        check_cancelled(ctx)?;
        self.redaction.apply_tabular(ctx, plan, tree).await?;
        check_cancelled(ctx)?;
        self.validation.apply_tabular(ctx, plan, tree).await?;
        check_cancelled(ctx)
    }

    pub(super) async fn run_image(
        &self,
        ctx: &RunContext,
        plan: &Plan,
        tree: &mut DocumentTree<Image>,
    ) -> Result<()> {
        check_cancelled(ctx)?;
        self.redaction.apply_image(ctx, plan, tree).await?;
        check_cancelled(ctx)?;
        self.validation.apply_image(ctx, plan, tree).await?;
        check_cancelled(ctx)
    }

    pub(super) async fn run_audio(
        &self,
        ctx: &RunContext,
        plan: &Plan,
        tree: &mut DocumentTree<Audio>,
    ) -> Result<()> {
        check_cancelled(ctx)?;
        self.redaction.apply_audio(ctx, plan, tree).await?;
        check_cancelled(ctx)?;
        self.validation.apply_audio(ctx, plan, tree).await?;
        check_cancelled(ctx)
    }
}

fn check_cancelled(ctx: &RunContext) -> Result<()> {
    if ctx.is_cancelled() {
        return Err(Error::cancellation("redaction cancelled", TARGET));
    }
    Ok(())
}
