//! Policy types: authored vocabulary for redaction governance.
//!
//! A [`Policy<M>`] is a named, versioned governance artefact: an
//! ordered list of [`PolicyRule`]s plus an optional fallback
//! [`Policy::default_action`] plus a retention configuration.
//! Policies are reusable — the same policy can participate in many
//! runs.
//!
//! Per-run composition (which policies apply to *this* run, in what
//! order) lives in the engine; the ontology does not model it.
//! Precedence is positional: in a run, the first policy in the
//! caller-supplied list is highest precedence; within a policy, the
//! first matching rule wins; the policy's `default_action` fires
//! only when no rule in that policy matched.

mod condition;
pub mod redaction;
mod retention;
mod rule;
mod selector;

use derive_builder::Builder;
use schemars::JsonSchema;
use semver::Version;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub use self::condition::Condition;
pub use self::redaction::AnyRedaction;
pub use self::retention::{Retention, RetentionPolicy, RetentionScope};
pub use self::rule::{Action, PolicyRule};
pub use self::selector::EntitySelector;
use crate::modality::DocumentModality;

/// A named, versioned governance policy for one modality.
#[derive(Debug, Clone, Builder, Serialize, Deserialize, JsonSchema)]
#[builder(
    name = "PolicyBuilder",
    pattern = "owned",
    setter(into, strip_option, prefix = "with")
)]
#[serde(rename_all = "camelCase")]
pub struct Policy<M: DocumentModality> {
    /// Unique identifier for this policy.
    #[builder(default = "Uuid::now_v7()")]
    pub id: Uuid,
    /// Human-readable policy name.
    pub name: String,
    /// Policy version.
    #[schemars(with = "String")]
    pub version: Version,
    /// Description of the policy's purpose.
    #[builder(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Ordered list of rules. First matching rule wins.
    #[builder(default = "Vec::new()")]
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rules: Vec<PolicyRule<M>>,
    /// Fallback action for entities that no [`PolicyRule`] in this
    /// policy matched. Consulted only after every rule in this
    /// policy has been considered; the engine then moves to the next
    /// policy in the per-run chain. `None` means "this policy has no
    /// opinion for unmatched entities; let the next policy decide."
    ///
    /// Authored on its own table — e.g. `[defaultAction]
    /// action = "redact"` — so the same TOML shape works as for
    /// rules.
    #[builder(default, setter(into = false))]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_action: Option<Action<M>>,
    /// Data retention lifecycle rules.
    #[builder(default = "Vec::new()")]
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub retention: Vec<RetentionPolicy>,
}

impl<M: DocumentModality> Policy<M> {
    /// Start building a new policy.
    pub fn builder() -> PolicyBuilder<M> {
        PolicyBuilder::default()
    }
}

/// Position of a winning rule inside a per-run policy chain.
///
/// Lexicographic order: lower wins. `policy_index` is the position of
/// the producing policy in the caller-supplied chain (`0` is highest
/// precedence). `rule_index` is the position of the producing rule
/// inside that policy (`0` is the first rule). [`Self::default`]
/// returns `policy_index = u32::MAX, rule_index = u32::MAX`, used
/// when a default fires after every rule in every layer was
/// considered.
///
/// Snapshotted into [`AuditEntry`] at evaluation time and carried
/// through to codec redactions so merge conflicts at overlap time
/// can be broken by "which rule fired this redaction."
///
/// [`AuditEntry`]: crate::document::provenance::AuditEntry
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RuleRank {
    /// 0-based position of the policy in the per-run chain.
    pub policy_index: u32,
    /// 0-based position of the rule inside its policy. A
    /// [`Policy::default_action`] firing uses `u32::MAX` so it
    /// sorts after every concrete rule in the same policy.
    pub rule_index: u32,
}

impl RuleRank {
    /// Construct a rank from the producing policy and rule indices.
    pub fn new(policy_index: u32, rule_index: u32) -> Self {
        Self {
            policy_index,
            rule_index,
        }
    }

    /// Rank for a [`Policy::default_action`] firing — sorts after
    /// every concrete rule in the same policy.
    pub fn for_default(policy_index: u32) -> Self {
        Self {
            policy_index,
            rule_index: u32::MAX,
        }
    }
}

impl Default for RuleRank {
    /// Returns the lowest-precedence rank (`u32::MAX` on both axes),
    /// suitable as a sentinel when no rule produced the decision.
    fn default() -> Self {
        Self {
            policy_index: u32::MAX,
            rule_index: u32::MAX,
        }
    }
}
