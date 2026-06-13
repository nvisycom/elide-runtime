//! [`Action<M>`] and [`PolicyRule<M>`] — what a policy rule does when
//! it matches an entity, plus the rule wrapper that binds a selector,
//! action, conditions, and enabled flag.

use hipstr::HipStr;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::condition::Condition;
use super::selector::EntitySelector;
use crate::modality::DocumentModality;

/// What a policy rule does when its selector matches an entity.
///
/// `Redact` carries the operator spec ([`DocumentModality::Redaction`])
/// the redaction phase instantiates per entity. The closed
/// per-modality enum (e.g. `TextRedaction`) carries the inline params
/// for built-in operators and a `Custom(AnonymizerId<M>)` arm that
/// resolves through the toolkit-side [`RedactionRegistry<M>`].
///
/// [`DocumentModality::Redaction`]: crate::modality::DocumentModality::Redaction
/// [`RedactionRegistry<M>`]: nvisy_toolkit::redaction::RedactionRegistry
#[derive(Debug, Clone, PartialEq)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum Action<M: DocumentModality> {
    /// Redact the matched entity using the given operator spec.
    Redact {
        /// Operator spec the redaction phase instantiates per entity.
        operator: M::Redaction,
    },
    /// Suppress a detection (treat as false positive). The entity is
    /// not redacted; an audit entry records the suppression.
    Suppress,
}

/// One rule inside a [`Policy`]: a name, a selector, an action,
/// optional conditions, and an enabled flag.
///
/// Rules are ordered inside their owning policy; the first matching
/// rule wins. There is no separate `priority` field — re-ordering
/// rules in the policy file is how authors change priority. The
/// [`name`] field is what audit entries reference; it must be
/// unique within the owning policy.
///
/// [`Policy`]: super::Policy
/// [`name`]: Self::name
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PolicyRule<M: DocumentModality> {
    /// Author-supplied rule name. Must be unique within the
    /// owning policy; audit entries reference this string verbatim.
    #[schemars(with = "String")]
    pub name: HipStr<'static>,
    /// Which entities this rule applies to.
    pub selector: EntitySelector,
    /// What this rule does when it matches. Flattened so the
    /// `action = "..."` discriminator and (for `Redact`) the
    /// `operator = ...` payload sit at the rule's top level — the
    /// shape author-facing TOML reads most naturally.
    #[serde(flatten)]
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
