//! Evaluate a [`Policy::applies_when`]
//! [`DocumentPredicate`] against the merged per-doc fact set.
//!
//! Document facts come from two sources, merged at evaluation
//! time: the doc's caller-supplied descriptor (labels +
//! metadata), and the per-request metadata from
//! [`StartBatch::metadata`]. Per-request keys override descriptor
//! keys on conflict (per the design decision: "explicit-per-request
//! wins").
//!
//! [`Policy::applies_when`]: nvisy_core::policy::Policy::applies_when
//! [`DocumentPredicate`]: nvisy_core::policy::DocumentPredicate
//! [`StartBatch::metadata`]: super::input::StartBatch::metadata

use std::collections::HashMap;

use nvisy_core::policy::{DocumentPredicate, Policy};

/// The merged per-document fact set [`DocumentPredicate`]
/// evaluates against.
pub(crate) struct DocumentFacts<'a> {
    /// Labels the descriptor carries (doc-level tags assigned at
    /// upload time).
    pub labels: &'a [String],
    /// Metadata: descriptor metadata + per-request metadata
    /// already merged with per-request keys winning conflicts.
    pub metadata: &'a HashMap<String, String>,
}

/// `true` when `predicate` holds against `facts`. Returns `true`
/// unconditionally when `predicate` is `None` (no gate).
pub(crate) fn policy_applies(policy: &Policy, facts: &DocumentFacts<'_>) -> bool {
    match &policy.applies_when {
        None => true,
        Some(predicate) => eval(predicate, facts),
    }
}

fn eval(predicate: &DocumentPredicate, facts: &DocumentFacts<'_>) -> bool {
    match predicate {
        DocumentPredicate::HasLabel { label } => facts.labels.iter().any(|l| l == label),
        DocumentPredicate::HasMetadata { key, value } => match facts.metadata.get(key) {
            Some(actual) => match value {
                Some(expected) => actual == expected,
                None => true,
            },
            None => false,
        },
        DocumentPredicate::All { all } => all.iter().all(|p| eval(p, facts)),
        DocumentPredicate::Any { any } => any.iter().any(|p| eval(p, facts)),
        DocumentPredicate::Not { not } => !eval(not, facts),
    }
}

/// Merge per-doc descriptor metadata with per-request metadata.
/// Per-request keys win on conflict.
pub(crate) fn merge_metadata(
    descriptor: &HashMap<String, String>,
    request: &HashMap<String, String>,
) -> HashMap<String, String> {
    let mut merged = descriptor.clone();
    for (k, v) in request {
        merged.insert(k.clone(), v.clone());
    }
    merged
}
