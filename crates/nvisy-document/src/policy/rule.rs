//! [`Action`] and [`PolicyRule`] — what a policy rule does when it
//! matches an entity, plus the rule wrapper that binds a selector,
//! action, conditions, and enabled flag.

use nvisy_toolkit::redaction::Redactable;
use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use super::condition::Condition;
use super::selector::EntitySelector;

/// The action a policy rule performs when it matches an entity.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(
    tag = "action",
    rename_all = "snake_case",
    bound(
        serialize = "M::Strategy: Serialize",
        deserialize = "M::Strategy: DeserializeOwned",
    )
)]
#[schemars(bound = "M::Strategy: JsonSchema")]
pub enum Action<M: Redactable> {
    /// Apply a redaction to the matched entity.
    Redact {
        /// Redaction strategy to apply.
        strategy: M::Strategy,
    },
    /// Suppress a detection (treat as false positive). The entity is
    /// not redacted; an audit entry records the suppression.
    Suppress,
}

/// One rule inside a [`Policy`]: a selector, an action, optional
/// conditions, and an enabled flag.
///
/// Rules are ordered inside their owning policy; the first matching
/// rule wins. There is no separate `priority` field — re-ordering
/// rules in the policy file is how authors change priority.
///
/// [`Policy`]: super::Policy
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(
    rename_all = "camelCase",
    bound(
        serialize = "M::Strategy: Serialize",
        deserialize = "M::Strategy: DeserializeOwned",
    )
)]
#[schemars(bound = "M::Strategy: JsonSchema")]
pub struct PolicyRule<M: Redactable> {
    /// Which entities this rule applies to.
    pub selector: EntitySelector,
    /// What this rule does when it matches.
    pub action: Action<M>,
    /// Conditions that must all be met for this rule to apply.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conditions: Vec<Condition>,
    /// Whether this rule is active.
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

fn default_enabled() -> bool {
    true
}
