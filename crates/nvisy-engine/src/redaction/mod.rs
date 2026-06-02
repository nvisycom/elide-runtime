//! Redaction: policy evaluation + multimodal application.
//!
//! Phase 4 of the pipeline. Two steps:
//!
//! 1. **Evaluate**: match entities against policy rules to produce
//!    [`AuditEntry`]s.
//! 2. **Apply**: build per-modality codec instructions (text, image,
//!    audio) from decisions and apply them to the document, writing
//!    replacement values into audit records.
//!
//! Unlike extraction and detection, redaction has no expensive
//! per-run construction — there's no model to load or HTTP client
//! to set up. The [`RedactionConfig`] config supplies deployment-wide
//! fallback values for plan [`Redaction`] fields that aren't
//! explicitly set.
//!
//! [`AuditEntry`]: nvisy_ontology::provenance::AuditEntry

mod apply;
mod evaluate;
mod strategy;

use nvisy_core::Result;
use nvisy_core::content::ContentMetadata;
use nvisy_ontology::document::Document;
use nvisy_ontology::entity::is_excluded;
use nvisy_ontology::modality::{Modality, Overlap};
use nvisy_ontology::primitive::ConfidenceThreshold;
use tracing::Instrument;

use self::evaluate::TARGET;
pub use self::evaluate::{ApplyRedactions, ApplyRedactionsImpl};
use crate::core::{
    DocumentTree, DocumentView, NodeMut, PolicyStore, RunContext, SharedHandle, ValueAt,
};
use crate::pipeline::{EngineInput, RedactionConfig};

/// Redaction phase: evaluate policies, attach an [`AuditEntry<M>`]
/// to each [`EntityRecord<M>`] the policy chain decides on, then
/// hand off to the codec applicator. Holds a [`RedactionConfig`]
/// by value for the deployment-wide defaults the policy chain
/// falls back to.
///
/// [`AuditEntry<M>`]: nvisy_ontology::provenance::AuditEntry
/// [`EntityRecord<M>`]: nvisy_ontology::provenance::EntityRecord
pub struct RedactionPhase {
    config: RedactionConfig,
}

impl RedactionPhase {
    /// Build the phase with the supplied config, used by
    /// [`DocumentPipeline::from_context`].
    ///
    /// [`DocumentPipeline::from_context`]: crate::pipeline::DocumentPipeline::from_context
    pub(crate) fn new(config: RedactionConfig) -> Self {
        Self { config }
    }

    /// Walk the tree and run the per-node redaction body. Skipped
    /// entirely when the orchestrator omits the phase (dry-run).
    /// Visits the root first, then iterates nested embedded
    /// documents; each per-node body borrows the policies, handle,
    /// and metadata directly from this scope.
    pub(crate) async fn apply(
        &self,
        ctx: &RunContext,
        input: &EngineInput,
        tree: &mut DocumentTree,
    ) -> Result<()> {
        let span = tracing::info_span!(target: TARGET, "phase", name = "redaction");
        let cfg = &input.plan.redaction;
        let default_threshold = cfg
            .confidence_threshold
            .unwrap_or(self.config.confidence_threshold);
        let _process_metadata = cfg.process_metadata.unwrap_or(self.config.process_metadata);
        let policies = &ctx.shared().policies;
        // Snapshot the tree-owned handle + metadata so they don't
        // conflict with the per-node `&mut` borrows produced by
        // `root_mut` / `embeds_mut` further down.
        let handle = tree.handle.clone();
        let metadata = tree.metadata.clone();
        async move {
            dispatch(
                tree.root_mut(),
                &handle,
                &metadata,
                policies,
                default_threshold,
            )
            .await?;
            for node in tree.embeds_mut() {
                dispatch(node, &handle, &metadata, policies, default_threshold).await?;
            }
            Ok(())
        }
        .instrument(span)
        .await
    }
}

async fn dispatch(
    node: NodeMut<'_>,
    handle: &SharedHandle,
    metadata: &ContentMetadata,
    policies: &PolicyStore,
    default_threshold: ConfidenceThreshold,
) -> Result<()> {
    match node {
        NodeMut::Text(doc) => {
            run_redaction(default_threshold, doc, handle, metadata, policies).await
        }
        NodeMut::Tabular(doc) => {
            run_redaction(default_threshold, doc, handle, metadata, policies).await
        }
        NodeMut::Image(doc) => {
            run_redaction(default_threshold, doc, handle, metadata, policies).await
        }
        NodeMut::Audio(doc) => {
            run_redaction(default_threshold, doc, handle, metadata, policies).await
        }
    }
}

/// Body of the redaction phase, parameterised on the resolved
/// `default_threshold`. Split out as a free function so the
/// test-only path can drive it directly.
pub(crate) async fn run_redaction<M>(
    default_threshold: ConfidenceThreshold,
    doc: &mut Document<M>,
    handle: &SharedHandle,
    metadata: &ContentMetadata,
    policies: &PolicyStore,
) -> Result<()>
where
    M: Modality + Overlap,
    for<'a> DocumentView<'a, M>: ValueAt<M>,
    ApplyRedactionsImpl: ApplyRedactions<M>,
{
    if doc.audit.records.is_empty() {
        return Ok(());
    }

    // Drop entities that overlap an Assert-strength Exclusion
    // annotation. Defence in depth: catches both well-meaning
    // detectors and LLMs that ignored the exclusion-hint prompt.
    let before_filter = doc.audit.records.len();
    let annotations = std::mem::take(&mut doc.annotations);
    doc.audit
        .records
        .retain(|record| !is_excluded(&annotations, &record.entity));
    doc.annotations = annotations;
    let dropped = before_filter - doc.audit.records.len();
    if dropped > 0 {
        tracing::debug!(
            target: TARGET,
            dropped,
            "filtered entities by Assert exclusion annotations",
        );
    }

    if doc.audit.records.is_empty() {
        return Ok(());
    }

    let document_labels: Vec<&str> = doc.labels.iter().map(|l| l.label.as_str()).collect();

    let mut records = std::mem::take(&mut doc.audit.records);
    evaluate::evaluate::<M>(
        &mut records,
        default_threshold,
        &document_labels,
        metadata,
        policies,
    )
    .await;
    doc.audit.records = records;

    tracing::debug!(
        target: TARGET,
        entries = doc.audit.entries().count(),
        "policy evaluation complete",
    );

    <ApplyRedactionsImpl as ApplyRedactions<M>>::apply_pending(doc, handle).await?;
    Ok(())
}
