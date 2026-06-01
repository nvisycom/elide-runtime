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
mod config;
mod evaluate;
mod plan;
mod strategy;

use std::sync::Arc;

use nvisy_core::Result;
use nvisy_ontology::entity::is_excluded;
use nvisy_ontology::modality::Modality;
use tracing::Instrument;

pub use self::config::RedactionConfig;
use self::evaluate::TARGET;
pub use self::evaluate::{ApplyRedactions, ApplyRedactionsImpl};
pub use self::plan::Redaction;
use crate::core::{DocView, DocumentTree, NodeMut, PolicyStore, RunContext, SharedHandle, ValueAt};
use crate::pipeline::EngineInput;

/// Redaction phase: evaluate policies, attach an [`AuditEntry<M>`]
/// to each [`EntityRecord<M>`] the policy chain decides on, then
/// hand off to the codec applicator. Holds the shared
/// [`RedactionConfig`] for default lookups.
///
/// [`AuditEntry<M>`]: nvisy_ontology::provenance::AuditEntry
/// [`EntityRecord<M>`]: nvisy_ontology::provenance::EntityRecord
pub struct RedactionPhase {
    config: Arc<RedactionConfig>,
}

impl RedactionPhase {
    /// Build the phase. The config comes from the run context; the
    /// phase stores its own `Arc` so the body doesn't have to
    /// re-thread it from `ctx` each call.
    pub fn new() -> Self {
        // Default empty config; the orchestrator overrides via
        // `from_context` (see [`DocumentPipeline::from_context`]).
        Self {
            config: Arc::new(RedactionConfig::default()),
        }
    }

    /// Build with the supplied config, used by
    /// [`DocumentPipeline::from_context`].
    ///
    /// [`DocumentPipeline::from_context`]: crate::pipeline::DocumentPipeline::from_context
    pub(crate) fn with_config(config: Arc<RedactionConfig>) -> Self {
        Self { config }
    }

    /// Walk the tree and run the per-node redaction body. Skipped
    /// entirely when the orchestrator omits the phase (dry-run).
    pub(crate) async fn apply(
        &self,
        ctx: &RunContext,
        input: &EngineInput,
        tree: &mut DocumentTree,
    ) -> Result<()> {
        let span = tracing::info_span!(target: TARGET, "phase", name = "redaction");
        let cfg = &input.plan.redaction;
        let section = self.config.as_ref();
        let default_threshold = cfg
            .confidence_threshold
            .unwrap_or(section.confidence_threshold);
        let _process_metadata = cfg.process_metadata.unwrap_or(section.process_metadata);

        let handle = tree.handle.clone();
        let metadata = Arc::new(tree.metadata.clone());
        let shared = Arc::clone(ctx.shared());
        async move {
            tree.walk_mut(move |node| {
                let handle = handle.clone();
                let metadata = Arc::clone(&metadata);
                let shared = Arc::clone(&shared);
                Box::pin(async move {
                    dispatch(
                        node,
                        &handle,
                        &metadata,
                        &shared.policies,
                        default_threshold,
                    )
                    .await
                })
            })
            .await
        }
        .instrument(span)
        .await
    }
}

impl Default for RedactionPhase {
    fn default() -> Self {
        Self::new()
    }
}

async fn dispatch(
    node: NodeMut<'_>,
    handle: &SharedHandle,
    metadata: &nvisy_core::content::ContentMetadata,
    policies: &PolicyStore,
    default_threshold: nvisy_ontology::primitive::ConfidenceThreshold,
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
    default_threshold: nvisy_ontology::primitive::ConfidenceThreshold,
    doc: &mut nvisy_ontology::document::Document<M>,
    handle: &SharedHandle,
    metadata: &nvisy_core::content::ContentMetadata,
    policies: &PolicyStore,
) -> Result<()>
where
    M: Modality + nvisy_ontology::modality::Overlap,
    for<'a> DocView<'a, M>: ValueAt<M>,
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
