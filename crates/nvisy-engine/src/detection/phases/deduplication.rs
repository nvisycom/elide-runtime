//! [`DeduplicationPhase`]: per-document dedup driver.
//!
//! Runs the canonical dedup pipeline (calibrate → filter → fuse →
//! resolve) via [`LayerPipeline::from_params`] against each
//! [`DocumentTree<M>`]'s audit records. Stateless; per-run config
//! comes from `plan` each call.
//!
//! [`LayerPipeline::from_params`]: nvisy_toolkit::deduplication::LayerPipeline::from_params

use std::mem;

use nvisy_core::Result;
use nvisy_core::entity::Entity;
use nvisy_core::extraction::TextAt;
use nvisy_core::modality::{Audio, Image, Overlap, Tabular, Text};
use nvisy_toolkit::deduplication::{LayerContext, LayerPipeline, SpanSize};
use tracing::Instrument;
use uuid::Uuid;

use crate::core::{DetectionContext, DocumentTree, PhaseContext as _};
use crate::detection::{DeduplicationParams, DetectionPlan};
use crate::document::provenance::EntityRecord;
use crate::modality::DocumentModality;

const TARGET: &str = "nvisy_engine::deduplication";

/// Deduplication phase orchestrator.
///
/// Stateless. Reads [`DeduplicationParams`] from `plan` each call.
pub struct DeduplicationPhase;

impl DeduplicationPhase {
    /// Build a fresh dedup phase. The phase is stateless; instances
    /// are interchangeable.
    pub fn new() -> Self {
        Self
    }

    pub(crate) async fn apply_text(
        &self,
        ctx: &DetectionContext,
        plan: &DetectionPlan,
        tree: &mut DocumentTree<Text>,
    ) -> Result<()> {
        self.run(ctx, plan, tree).await
    }

    pub(crate) async fn apply_tabular(
        &self,
        ctx: &DetectionContext,
        plan: &DetectionPlan,
        tree: &mut DocumentTree<Tabular>,
    ) -> Result<()> {
        self.run(ctx, plan, tree).await
    }

    pub(crate) async fn apply_image(
        &self,
        ctx: &DetectionContext,
        plan: &DetectionPlan,
        tree: &mut DocumentTree<Image>,
    ) -> Result<()> {
        self.run(ctx, plan, tree).await
    }

    pub(crate) async fn apply_audio(
        &self,
        ctx: &DetectionContext,
        plan: &DetectionPlan,
        tree: &mut DocumentTree<Audio>,
    ) -> Result<()> {
        self.run(ctx, plan, tree).await
    }

    async fn run<M>(
        &self,
        ctx: &DetectionContext,
        plan: &DetectionPlan,
        tree: &mut DocumentTree<M>,
    ) -> Result<()>
    where
        M: DocumentModality,
        M::Location: Overlap + SpanSize,
        DocumentTree<M>: TextAt<M>,
    {
        let span = tracing::info_span!(target: TARGET, "phase", name = "deduplication");
        let run_id = ctx.shared().run_id;
        async move { dedup_one(tree, &plan.deduplication, run_id).await }
            .instrument(span)
            .await
    }
}

impl Default for DeduplicationPhase {
    fn default() -> Self {
        Self::new()
    }
}

async fn dedup_one<M>(
    tree: &mut DocumentTree<M>,
    dedup: &DeduplicationParams,
    run_id: Uuid,
) -> Result<()>
where
    M: DocumentModality,
    M::Location: Overlap + SpanSize,
    DocumentTree<M>: TextAt<M>,
{
    if tree.root.audit.records.is_empty() {
        return Ok(());
    }
    tracing::debug!(
        target: TARGET,
        entities = tree.root.audit.records.len(),
        "running deduplication",
    );
    // Dedup runs before redaction evaluation, so every record's
    // `audit` is still None; we can pull entities out, dedup, and
    // rewrap without losing audit state.
    let records = mem::take(&mut tree.root.audit.records);
    let entities: Vec<Entity<M>> = records.into_iter().map(|r| r.entity).collect();
    let pipeline: LayerPipeline<M, _> = LayerPipeline::from_params(dedup);
    let ctx = LayerContext::new(&*tree).with_correlation_id(run_id);
    let deduped = pipeline.run(entities, &ctx).await;
    tree.root.audit.records = deduped.into_iter().map(EntityRecord::new).collect();
    Ok(())
}
