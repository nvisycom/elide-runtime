//! Post-redaction validation: per-modality leak detection.
//!
//! Public surface is the [`CheckLeaks`] trait plus the
//! [`LeakedValue`] / [`ValidationResult`] result types and the
//! [`OnLeak`] / [`Validation`] plan types. The phase orchestrator
//! that walks the tree and dispatches leak checks lives in
//! [`ValidationPhase`](crate::pipeline::ValidationPhase).
//!
//! Per-modality leak detection lives behind the [`CheckLeaks`]
//! trait — Text and Tabular run real substring checks; Image and
//! Audio currently return [`ValidationResult::skipped`] because
//! visual / audio inspection isn't implemented yet.

mod check;
mod plan;

use uuid::Uuid;

pub use self::check::CheckLeaks;
pub use self::plan::{OnLeak, Validation};

/// A sensitive value that was not properly redacted.
#[derive(Debug, Clone)]
pub struct LeakedValue {
    pub value: String,
    pub entity_id: Uuid,
}

/// Result of validation for one node.
///
/// `skipped` is `true` when the modality has no leak-detection
/// implementation (Image, Audio). In that case `passed` and `leaked`
/// are both empty / zero — no claim is made either way.
#[derive(Debug)]
pub struct ValidationResult {
    pub passed: usize,
    pub leaked: Vec<LeakedValue>,
    pub skipped: bool,
}

impl ValidationResult {
    /// Returned by per-modality checks that don't run any inspection
    /// (Image, Audio today).
    pub fn skipped() -> Self {
        Self {
            passed: 0,
            leaked: Vec::new(),
            skipped: true,
        }
    }
}
