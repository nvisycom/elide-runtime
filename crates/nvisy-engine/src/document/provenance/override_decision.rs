//! [`RedactionDecision`]: per-entity provenance recorded on an
//! [`AuditEntry`] when a redaction pass applied a human override.
//!
//! The value is stamped onto [`EntryMetadata::override_decision`]
//! during [`RedactionEngine::redact`]. Reviewers inspecting the final
//! audit can distinguish a policy-chain decision from a
//! human-mediated one.
//!
//! Lives in `provenance/` rather than `pipeline/redaction/` because
//! it's a property of the audit, not of the redaction-request
//! shape. The redaction module re-exports it for ergonomic access.
//!
//! [`AuditEntry`]: super::AuditEntry
//! [`EntryMetadata::override_decision`]: super::EntryMetadata::override_decision
//! [`RedactionEngine::redact`]: crate::redaction::RedactionEngine::redact

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Provenance tag for an [`AuditEntry`] decision.
///
/// `PolicyChain` means the policy chain's choice stands and no
/// human touched this entity. The four `Override*` variants
/// correspond 1:1 with the override types
/// [`RedactionOverride`] accepts.
///
/// [`AuditEntry`]: super::AuditEntry
/// [`RedactionOverride`]: crate::redaction::RedactionOverride
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RedactionDecision {
    /// Default — no override touched this entity; the policy
    /// chain's choice stands.
    PolicyChain,
    /// A `RedactionOverride::Accept` explicitly approved the
    /// policy chain's choice. Distinct from `PolicyChain` so
    /// reviewers can see "this was reviewed" vs "this was
    /// untouched."
    OverrideAccept,
    /// A `RedactionOverride::Reject` suppressed this entity.
    OverrideReject,
    /// A `RedactionOverride::Replace` swapped the operator.
    OverrideReplace,
    /// A `RedactionOverride::Add` introduced this entity; the
    /// recognisers did not detect it. Policy evaluation ran
    /// against the synthesised entity exactly as it would for a
    /// recogniser-detected one.
    OverrideAdd,
}
