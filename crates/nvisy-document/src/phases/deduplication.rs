//! [`DeduplicationPhase`]: per-document dedup driver.
//!
//! Runs the canonical dedup pipeline (calibrate → filter → fuse →
//! resolve) via [`LayerPipeline::from_params`] against each
//! [`DocumentTree<M>`]'s audit records. Stateless; per-run config
//! comes from `input.plan` each call.
//!
//! [`LayerPipeline::from_params`]: nvisy_toolkit::deduplication::LayerPipeline::from_params

use std::mem;

use nvisy_core::Result;
use nvisy_core::entity::Entity;
use nvisy_core::extraction::TextAt;
use nvisy_core::modality::{Audio, Image, Overlap, Tabular, Text};
use nvisy_toolkit::deduplication::{FilterParams, LayerContext, LayerPipeline, SpanSize};
use tracing::Instrument;
use uuid::Uuid;

use crate::core::{DocumentTree, RunContext};
use crate::modality::DocumentModality;
use crate::pipeline::{DeduplicationParams, Detection, EngineInput};
use crate::provenance::EntityRecord;

const TARGET: &str = "nvisy_document::deduplication";

/// Deduplication phase orchestrator.
///
/// Stateless. Per-run config ([`DeduplicationParams`] for
/// calibration/threshold/grouping, [`Detection`] for the
/// allowed-kinds list) comes from `input.plan` each call.
pub struct DeduplicationPhase;

impl DeduplicationPhase {
    pub fn new() -> Self {
        Self
    }

    pub(crate) async fn apply_text(
        &self,
        ctx: &RunContext,
        input: &EngineInput,
        tree: &mut DocumentTree<Text>,
    ) -> Result<()> {
        self.run(ctx, input, tree).await
    }

    pub(crate) async fn apply_tabular(
        &self,
        ctx: &RunContext,
        input: &EngineInput,
        tree: &mut DocumentTree<Tabular>,
    ) -> Result<()> {
        self.run(ctx, input, tree).await
    }

    pub(crate) async fn apply_image(
        &self,
        ctx: &RunContext,
        input: &EngineInput,
        tree: &mut DocumentTree<Image>,
    ) -> Result<()> {
        self.run(ctx, input, tree).await
    }

    pub(crate) async fn apply_audio(
        &self,
        ctx: &RunContext,
        input: &EngineInput,
        tree: &mut DocumentTree<Audio>,
    ) -> Result<()> {
        self.run(ctx, input, tree).await
    }

    async fn run<M>(
        &self,
        ctx: &RunContext,
        input: &EngineInput,
        tree: &mut DocumentTree<M>,
    ) -> Result<()>
    where
        M: DocumentModality,
        M::Location: Overlap + SpanSize,
        DocumentTree<M>: TextAt<M>,
    {
        let span = tracing::info_span!(target: TARGET, "phase", name = "deduplication");
        let run_id = ctx.shared().run_id;
        async move {
            dedup_one(
                tree,
                &input.plan.deduplication,
                &input.plan.detection,
                run_id,
            )
            .await
        }
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
    detection: &Detection,
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
    let filter = FilterParams {
        allowed_kinds: (!detection.entity_kinds.is_empty()).then(|| detection.entity_kinds.clone()),
        confidence_threshold: dedup.confidence_threshold,
    };
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
    let pipeline: LayerPipeline<M, _> = LayerPipeline::from_params(dedup, filter);
    let ctx = LayerContext::new(&*tree).with_correlation_id(run_id);
    let deduped = pipeline.run(entities, &ctx).await;
    tree.root.audit.records = deduped.into_iter().map(EntityRecord::new).collect();
    Ok(())
}
