//! [`DeduplicationPhase`]: per-node dedup driver.
//!
//! Walks the [`DocumentTree`], pulling each node's entity records,
//! running the four-step dedup pipeline (calibrate → filter → fuse →
//! resolve), and rewrapping the result. Stateless; per-run config
//! comes from `input.plan`.
//!
//! The actual algorithm lives in [`crate::deduplication::deduplicate`];
//! this phase is purely the document-traversal glue.
//!
//! [`DocumentTree`]: crate::core::DocumentTree

use std::mem;

use nvisy_core::Result;
use nvisy_ontology::document::Document;
use nvisy_ontology::entity::Entity;
use nvisy_ontology::modality::{Modality, Overlap};
use nvisy_ontology::provenance::EntityRecord;
use tracing::Instrument;

use crate::core::{DocumentTree, DocumentView, NodeMut, RunContext, SharedHandle, ValueAt};
use crate::deduplication::{FilterParams, SpanSize, deduplicate};
use crate::pipeline::{DeduplicationParams, Detection, EngineInput};

const TARGET: &str = "nvisy_engine::deduplication";

/// Deduplication phase orchestrator.
///
/// Stateless. Per-run config ([`DeduplicationParams`] for
/// calibration/threshold/grouping, [`Detection`] for the
/// allowed-kinds list) comes from `input.plan` each call.
pub struct DeduplicationPhase;

impl DeduplicationPhase {
    /// Build the phase. Stateless.
    pub fn new() -> Self {
        Self
    }

    /// Walk the tree and run the per-node dedup body. Visits the root
    /// first, then iterates nested embedded documents; each per-node
    /// body borrows the detection + dedup plan and handle directly
    /// from this scope.
    pub(crate) async fn apply(
        &self,
        _ctx: &RunContext,
        input: &EngineInput,
        tree: &mut DocumentTree,
    ) -> Result<()> {
        let span = tracing::info_span!(target: TARGET, "phase", name = "deduplication");
        // Snapshot the tree-owned handle so it doesn't conflict with
        // the per-node `&mut` borrows produced by `root_mut` /
        // `embeds_mut` further down.
        let handle = tree.handle.clone();
        async move {
            dispatch(
                tree.root_mut(),
                &handle,
                &input.plan.deduplication,
                &input.plan.detection,
            )
            .await?;
            for node in tree.embeds_mut() {
                dispatch(
                    node,
                    &handle,
                    &input.plan.deduplication,
                    &input.plan.detection,
                )
                .await?;
            }
            Ok(())
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

async fn dispatch(
    node: NodeMut<'_>,
    handle: &SharedHandle,
    dedup: &DeduplicationParams,
    detection: &Detection,
) -> Result<()> {
    match node {
        NodeMut::Text(doc) => dedup_one(doc, handle, dedup, detection).await,
        NodeMut::Tabular(doc) => dedup_one(doc, handle, dedup, detection).await,
        NodeMut::Image(doc) => dedup_one(doc, handle, dedup, detection).await,
        NodeMut::Audio(doc) => dedup_one(doc, handle, dedup, detection).await,
    }
}

async fn dedup_one<M>(
    doc: &mut Document<M>,
    handle: &SharedHandle,
    dedup: &DeduplicationParams,
    detection: &Detection,
) -> Result<()>
where
    M: Modality + Overlap + SpanSize,
    for<'a> DocumentView<'a, M>: ValueAt<M>,
{
    if doc.audit.records.is_empty() {
        return Ok(());
    }
    let filter = FilterParams {
        allowed_kinds: (!detection.entity_kinds.is_empty()).then(|| detection.entity_kinds.clone()),
        confidence_threshold: dedup.confidence_threshold,
    };
    tracing::debug!(
        target: TARGET,
        entities = doc.audit.records.len(),
        "running deduplication",
    );
    // Dedup runs before redaction evaluation, so every record's
    // `audit` is still None; we can pull entities out, dedup, and
    // rewrap without losing audit state.
    let records = mem::take(&mut doc.audit.records);
    let entities: Vec<Entity<M>> = records.into_iter().map(|r| r.entity).collect();
    let deduped = deduplicate(dedup, &filter, entities, doc, handle).await;
    doc.audit.records = deduped.into_iter().map(EntityRecord::new).collect();
    Ok(())
}
