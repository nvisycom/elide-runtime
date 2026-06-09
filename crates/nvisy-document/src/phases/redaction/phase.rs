//! [`RedactionPhase`]: per-modality redaction driver.
//!
//! Resolves the per-run threshold from the plan (falling back to the
//! deployment-wide [`RedactionConfig`]), then drives the per-tree
//! `run_redaction` body in the parent module.
//!
//! Holds one [`RedactionRegistry<M>`] per modality, which the apply
//! step consults when a rule's operator spec is the `Custom` arm.
//! Built-in operators are constructed inline from the rule's spec
//! and don't touch the registry.
//!
//! [`RedactionRegistry<M>`]: nvisy_toolkit::redaction::RedactionRegistry

use nvisy_core::Result;
use nvisy_core::modality::{Audio, Image, Tabular, Text};

use crate::core::{DocumentTree, Plan, RunContext};
use crate::phases::redaction::registries::RedactionRegistries;
use crate::phases::redaction::run_redaction;
use crate::pipeline::RedactionConfig;

const TARGET: &str = "nvisy_document::redaction";

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
    pub(crate) fn new(config: RedactionConfig, registries: RedactionRegistries) -> Self {
        Self { config, registries }
    }

    pub(crate) async fn apply_text(
        &self,
        ctx: &RunContext,
        plan: &Plan,
        tree: &mut DocumentTree<Text>,
    ) -> Result<()> {
        let threshold = plan
            .redaction
            .confidence_threshold
            .unwrap_or(self.config.confidence_threshold);
        let policies = &ctx.shared().policies;
        let descriptor = tree.descriptor.clone();
        run_redaction(
            threshold,
            tree,
            &descriptor,
            policies,
            &self.registries.text,
        )
        .await
    }

    pub(crate) async fn apply_image(
        &self,
        ctx: &RunContext,
        plan: &Plan,
        tree: &mut DocumentTree<Image>,
    ) -> Result<()> {
        let threshold = plan
            .redaction
            .confidence_threshold
            .unwrap_or(self.config.confidence_threshold);
        let policies = &ctx.shared().policies;
        let descriptor = tree.descriptor.clone();
        run_redaction(
            threshold,
            tree,
            &descriptor,
            policies,
            &self.registries.image,
        )
        .await
    }

    pub(crate) async fn apply_audio(
        &self,
        ctx: &RunContext,
        plan: &Plan,
        tree: &mut DocumentTree<Audio>,
    ) -> Result<()> {
        let threshold = plan
            .redaction
            .confidence_threshold
            .unwrap_or(self.config.confidence_threshold);
        let policies = &ctx.shared().policies;
        let descriptor = tree.descriptor.clone();
        run_redaction(
            threshold,
            tree,
            &descriptor,
            policies,
            &self.registries.audio,
        )
        .await
    }

    /// Tabular redaction is not implemented yet — there's no
    /// `Anonymizer<Tabular>` in the workspace. Warn if records are
    /// pending and skip the apply step.
    pub(crate) async fn apply_tabular(
        &self,
        _ctx: &RunContext,
        _plan: &Plan,
        tree: &mut DocumentTree<Tabular>,
    ) -> Result<()> {
        let pending = tree.root.audit.records.len();
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
