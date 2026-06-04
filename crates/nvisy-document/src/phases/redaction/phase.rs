//! [`RedactionPhase`]: per-node redaction driver.
//!
//! Resolves the per-run threshold from the plan (falling back to the
//! deployment-wide [`RedactionConfig`]), then walks the
//! [`DocumentTree`] feeding each node to the crate-private
//! `run_redaction` dispatcher in the parent module.
//!
//! Holds one [`RedactionRegistry<M>`] per modality, which the apply
//! step consults when a rule's operator spec is the `Custom` arm.
//! Built-in operators are constructed inline from the rule's spec
//! and don't touch the registry.
//!
//! [`DocumentTree`]: crate::core::DocumentTree
//! [`RedactionRegistry<M>`]: nvisy_toolkit::redaction::RedactionRegistry

use nvisy_core::Result;
use nvisy_core::content::ContentMetadata;
use nvisy_core::primitive::ConfidenceThreshold;
use tracing::Instrument;

use crate::core::{DocumentTree, NodeMut, PolicyStore, RunContext, SharedHandle};
use crate::phases::redaction::registries::RedactionRegistries;
use crate::phases::redaction::run_redaction;
use crate::pipeline::{EngineInput, RedactionConfig};

const TARGET: &str = "nvisy_engine::redaction";

/// Redaction phase orchestrator. Holds a [`RedactionConfig`] for the
/// deployment-wide defaults and one [`RedactionRegistry<M>`] per
/// modality for `Custom`-arm lookups at apply time.
///
/// [`RedactionRegistry<M>`]: nvisy_toolkit::redaction::RedactionRegistry
pub struct RedactionPhase {
    config: RedactionConfig,
    registries: RedactionRegistries,
}

impl RedactionPhase {
    /// Build the phase with the supplied config and per-modality
    /// custom-operator registries.
    pub(crate) fn new(config: RedactionConfig, registries: RedactionRegistries) -> Self {
        Self { config, registries }
    }

    /// Walk the tree and run the per-node redaction body.
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
        let handle = tree.handle.clone();
        let metadata = tree.metadata.clone();
        let registries = &self.registries;
        async move {
            dispatch(
                tree.root_mut(),
                &handle,
                &metadata,
                policies,
                default_threshold,
                registries,
            )
            .await?;
            for node in tree.embeds_mut() {
                dispatch(
                    node,
                    &handle,
                    &metadata,
                    policies,
                    default_threshold,
                    registries,
                )
                .await?;
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
    registries: &RedactionRegistries,
) -> Result<()> {
    match node {
        NodeMut::Text(doc) => {
            run_redaction(
                default_threshold,
                doc,
                handle,
                metadata,
                policies,
                &registries.text,
            )
            .await
        }
        NodeMut::Image(doc) => {
            run_redaction(
                default_threshold,
                doc,
                handle,
                metadata,
                policies,
                &registries.image,
            )
            .await
        }
        NodeMut::Audio(doc) => {
            run_redaction(
                default_threshold,
                doc,
                handle,
                metadata,
                policies,
                &registries.audio,
            )
            .await
        }
        // Tabular has no `ModalityData` impl and no `Anonymizer<Tabular>`
        // exists in the workspace yet. Skip the node, but warn if the
        // policy resolver actually picked redactions for it — silent
        // drop would mask broken governance.
        NodeMut::Tabular(doc) => {
            let pending = doc.audit.records.len();
            if pending > 0 {
                tracing::warn!(
                    target: TARGET,
                    pending,
                    "tabular redaction not implemented; skipping {pending} audit record(s)",
                );
            }
            Ok(())
        }
    }
}
