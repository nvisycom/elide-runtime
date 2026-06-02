//! [`RedactionPhase`]: per-node redaction driver.
//!
//! Resolves the per-run threshold from the plan (falling back to the
//! deployment-wide [`RedactionConfig`]), then walks the
//! [`DocumentTree`] feeding each node to [`run_redaction`] — the
//! actual policy evaluation + codec application loop, which lives in
//! the redaction module proper.
//!
//! [`DocumentTree`]: crate::core::DocumentTree
//! [`run_redaction`]: crate::redaction::run_redaction

use nvisy_core::Result;
use nvisy_core::content::ContentMetadata;
use nvisy_ontology::primitive::ConfidenceThreshold;
use tracing::Instrument;

use crate::core::{DocumentTree, NodeMut, PolicyStore, RunContext, SharedHandle};
use crate::pipeline::{EngineInput, RedactionConfig};
use crate::redaction::run_redaction;

const TARGET: &str = "nvisy_engine::redaction";

/// Redaction phase orchestrator. Holds a [`RedactionConfig`] by value
/// for the deployment-wide defaults the plan falls back to.
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
