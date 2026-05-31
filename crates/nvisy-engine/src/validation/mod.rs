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

use std::marker::PhantomData;

use nvisy_core::{Error, Result};
use nvisy_ontology::modality::Modality;
use uuid::Uuid;

pub use self::check::CheckLeaks;
pub use self::plan::{OnLeak, Validation};
use crate::core::ValueAt;
use crate::pipeline::{ModalityKind, Phase, PhaseContext, PhaseInfo, PhaseTarget};

const TARGET: &str = "nvisy_engine::validation";

/// A sensitive value that was not properly redacted.
#[derive(Debug, Clone)]
pub struct LeakedValue {
    pub value: String,
    pub entity_id: Uuid,
}

/// Result of validation for one envelope.
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

/// Validation phase: re-scans the redacted envelope for leaked
/// values via per-modality [`CheckLeaks`] dispatch. Read-only against
/// the codec handle; surfaces failures via the `Result` when
/// `on_leak = OnLeak::Fail` is set in the plan config.
///
/// Stateless beyond the modality marker; per-call config comes from
/// `ctx.plan.validation` each call.
pub struct ValidationPhase<M: Modality> {
    _marker: PhantomData<fn() -> M>,
}

impl<M: Modality> ValidationPhase<M> {
    /// Build the phase. Stateless beyond the modality marker.
    pub fn new() -> Self {
        Self {
            _marker: PhantomData,
        }
    }
}

impl<M: Modality> Default for ValidationPhase<M> {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl<M> Phase<M> for ValidationPhase<M>
where
    M: Modality + CheckLeaks,
    for<'a> crate::core::DocView<'a, M>: ValueAt<M>,
{
    fn inspect(&self) -> PhaseInfo {
        PhaseInfo {
            name: "validation",
            modality: ModalityKind::of::<M>(),
            mutating: false,
        }
    }

    async fn run(&self, ctx: &PhaseContext<'_, M>, target: &mut PhaseTarget<'_, M>) -> Result<()> {
        let on_leak = ctx.plan.validation.on_leak;

        tracing::debug!(target: TARGET, "running post-redaction validation");

        let result = M::check_leaks(target.doc, target.handle).await;

        if result.skipped {
            tracing::debug!(
                target: TARGET,
                "validation skipped: no leak-detection implementation for this modality",
            );
            return Ok(());
        }

        if result.leaked.is_empty() {
            tracing::debug!(target: TARGET, passed = result.passed, "validation passed");
            return Ok(());
        }

        tracing::warn!(
            target: TARGET,
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
}
