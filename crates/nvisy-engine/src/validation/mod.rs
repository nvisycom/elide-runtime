//! Post-redaction validator.
//!
//! Re-scans redacted content to verify that no originally detected
//! values remain visible. Runs after [`Redactor`].
//!
//! Per-modality leak detection lives behind the [`CheckLeaks`]
//! trait — Text and Tabular run real substring checks; Image and
//! Audio currently return [`ValidationResult::skipped`] because
//! visual / audio inspection isn't implemented yet.
//!
//! [`Redactor`]: crate::redaction::Redactor

mod check;
mod workflow;

use nvisy_core::{Error, Result};
use uuid::Uuid;

pub use self::check::CheckLeaks;
pub use self::workflow::Validation;
use self::workflow::Validation as ValidationConfig;
use crate::envelope::DocumentEnvelope;

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

/// Post-redaction validator that checks for leaked sensitive values.
pub struct Validator {
    fail_on_leak: bool,
}

impl Validator {
    /// Create from config.
    pub fn new(cfg: &ValidationConfig) -> Self {
        Self {
            fail_on_leak: cfg.fail_on_leak,
        }
    }

    /// Execute post-redaction validation against the envelope.
    ///
    /// Generic over modality: dispatches through [`CheckLeaks`] to
    /// the per-modality implementation. Modalities without leak
    /// detection (Image, Audio) return early with a debug log;
    /// `fail_on_leak` only fires when an inspecting modality
    /// surfaces a leak.
    pub async fn execute<M>(&self, envelope: &mut DocumentEnvelope<M>) -> Result<()>
    where
        M: CheckLeaks,
    {
        tracing::debug!(target: TARGET, "running post-redaction validation");

        let result = M::check_leaks(envelope).await;

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

        if self.fail_on_leak {
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
