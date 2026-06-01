//! Post-redaction validation phase.
//!
//! Re-scans redacted content to verify that no originally detected
//! values remain visible. Runs after [`RedactionPhase`].
//!
//! Per-modality leak detection lives behind the [`CheckLeaks`]
//! trait — Text and Tabular run real substring checks; Image and
//! Audio currently return [`ValidationResult::skipped`] because
//! visual / audio inspection isn't implemented yet.
//!
//! [`RedactionPhase`]: crate::redaction::RedactionPhase

mod check;
mod plan;

use nvisy_core::{Error, Result};
use tracing::Instrument;
use uuid::Uuid;

pub use self::check::CheckLeaks;
pub use self::plan::{OnLeak, Validation};
use crate::core::{DocumentTree, NodeMut, RunContext, SharedHandle};
use crate::pipeline::EngineInput;

const TARGET: &str = "nvisy_engine::validation";

/// A sensitive value that was not properly redacted.
#[derive(Debug, Clone)]
pub struct LeakedValue {
    pub value: String,
    pub entity_id: Uuid,
}

/// Result of validation for one node.
///
/// `skipped` is `true` when the modality has no leak-detection
/// implementation (Image, Audio). In that case `passed` and
/// `leaked` are both empty / zero — no claim is made either way.
#[derive(Debug)]
pub struct ValidationResult {
    pub passed: usize,
    pub leaked: Vec<LeakedValue>,
    pub skipped: bool,
}

impl ValidationResult {
    /// Returned by per-modality checks that don't run any
    /// inspection (Image, Audio today).
    pub fn skipped() -> Self {
        Self {
            passed: 0,
            leaked: Vec::new(),
            skipped: true,
        }
    }
}

/// Validation phase: re-scans the redacted document for leaked
/// values via per-modality [`CheckLeaks`] dispatch. Read-only
/// against the codec handle; surfaces failures via the `Result`
/// when `on_leak = OnLeak::Fail` is set in the plan config.
pub struct ValidationPhase;

impl ValidationPhase {
    /// Build the phase. Stateless.
    pub fn new() -> Self {
        Self
    }

    /// Walk the tree and run leak detection per node.
    pub(crate) async fn apply(
        &self,
        _ctx: &RunContext,
        input: &EngineInput,
        tree: &mut DocumentTree,
    ) -> Result<()> {
        let span = tracing::info_span!(target: TARGET, "phase", name = "validation");
        let on_leak = input.plan.validation.on_leak;
        let handle = tree.handle.clone();
        async move {
            tree.walk_mut(move |node| {
                let handle = handle.clone();
                Box::pin(async move { dispatch(node, &handle, on_leak).await })
            })
            .await
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
        NodeMut::Text(doc) => (
            <nvisy_ontology::modality::Text as CheckLeaks>::check_leaks(doc, handle).await,
            "text",
        ),
        NodeMut::Tabular(doc) => (
            <nvisy_ontology::modality::Tabular as CheckLeaks>::check_leaks(doc, handle).await,
            "tabular",
        ),
        NodeMut::Image(doc) => (
            <nvisy_ontology::modality::Image as CheckLeaks>::check_leaks(doc, handle).await,
            "image",
        ),
        NodeMut::Audio(doc) => (
            <nvisy_ontology::modality::Audio as CheckLeaks>::check_leaks(doc, handle).await,
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
