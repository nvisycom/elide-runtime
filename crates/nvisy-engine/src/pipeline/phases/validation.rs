//! [`ValidationPhase`]: per-node leak-detection driver.
//!
//! Re-scans the redacted document for leaked values via per-modality
//! [`CheckLeaks`] dispatch. Read-only against the codec handle;
//! surfaces failures via the `Result` when `on_leak = OnLeak::Fail`
//! is set in the plan config.
//!
//! [`CheckLeaks`]: crate::validation::CheckLeaks

use nvisy_core::{Error, Result};
use nvisy_ontology::modality::{Audio, Image, Tabular, Text};
use tracing::Instrument;

use crate::core::{DocumentTree, NodeMut, RunContext, SharedHandle};
use crate::pipeline::EngineInput;
use crate::validation::{CheckLeaks, OnLeak};

const TARGET: &str = "nvisy_engine::validation";

/// Validation phase orchestrator.
pub struct ValidationPhase;

impl ValidationPhase {
    /// Build the phase. Stateless.
    pub fn new() -> Self {
        Self
    }

    /// Walk the tree and run leak detection per node. Visits the root
    /// first, then iterates nested embedded documents; each per-node
    /// body borrows the handle directly from this scope.
    pub(crate) async fn apply(
        &self,
        _ctx: &RunContext,
        input: &EngineInput,
        tree: &mut DocumentTree,
    ) -> Result<()> {
        let span = tracing::info_span!(target: TARGET, "phase", name = "validation");
        let on_leak = input.plan.validation.on_leak;
        // Snapshot the tree-owned handle so it doesn't conflict with
        // the per-node `&mut` borrows produced by `root_mut` /
        // `embeds_mut` further down.
        let handle = tree.handle.clone();
        async move {
            dispatch(tree.root_mut(), &handle, on_leak).await?;
            for node in tree.embeds_mut() {
                dispatch(node, &handle, on_leak).await?;
            }
            Ok(())
        }
        .instrument(span)
        .await
    }
}

impl Default for ValidationPhase {
    fn default() -> Self {
        Self::new()
    }
}

async fn dispatch(node: NodeMut<'_>, handle: &SharedHandle, on_leak: OnLeak) -> Result<()> {
    let (result, modality) = match node {
        NodeMut::Text(doc) => (<Text as CheckLeaks>::check_leaks(doc, handle).await, "text"),
        NodeMut::Tabular(doc) => (
            <Tabular as CheckLeaks>::check_leaks(doc, handle).await,
            "tabular",
        ),
        NodeMut::Image(doc) => (
            <Image as CheckLeaks>::check_leaks(doc, handle).await,
            "image",
        ),
        NodeMut::Audio(doc) => (
            <Audio as CheckLeaks>::check_leaks(doc, handle).await,
            "audio",
        ),
    };

    if result.skipped {
        tracing::debug!(
            target: TARGET,
            modality,
            "validation skipped: no leak-detection implementation for this modality",
        );
        return Ok(());
    }

    if result.leaked.is_empty() {
        tracing::debug!(
            target: TARGET,
            modality,
            passed = result.passed,
            "validation passed",
        );
        return Ok(());
    }

    tracing::warn!(
        target: TARGET,
        modality,
        leaked = result.leaked.len(),
        passed = result.passed,
        "validation found leaked values",
    );

    if matches!(on_leak, OnLeak::Fail) {
        let details: Vec<String> = result
            .leaked
            .iter()
            .map(|l| format!("{}({})", l.value, l.entity_id))
            .collect();
        return Err(Error::validation(
            format!(
                "{} redacted value(s) leaked in output: {}",
                result.leaked.len(),
                details.join(", "),
            ),
            "validation",
        ));
    }

    Ok(())
}
