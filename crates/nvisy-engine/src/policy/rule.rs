//! [`Action`] and [`PolicyRule`] — what a policy rule does when it
//! matches an entity, plus the rule wrapper that binds a selector,
//! action, conditions, and enabled flag.

use hipstr::HipStr;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::audit::AuditAction;
use super::condition::Condition;
use super::redaction::ModalityRedactions;
use super::selector::EntitySelector;
use super::suppress::SuppressAction;

/// What a policy rule does when its selector matches an entity.
///
/// Three verbs:
///
/// - [`Action::Redact`] transforms matching entities. Carries one
///   [`ModalityRedactions`] map: a rule can name operators for
///   multiple modalities at once, and the apply phase picks the
///   one matching the entity's modality.
/// - [`Action::Suppress`] drops matching entities (treat as a
///   false positive). Carries an optional `reason` propagated into
///   the audit entry.
/// - [`Action::Audit`] flags matching entities for human review
///   without transforming them. The detection pass already emits
///   an audit entry per matched entity; this action tags those
///   entries with a severity hint downstream review tooling can
///   prioritise on.
///
/// Wire shape uses an externally-tagged enum with the variant name
/// as the discriminator key — `[policies.rules.redact]`,
/// `[policies.rules.suppress]`, `[policies.rules.audit]` — flattened
/// onto the parent [`PolicyRule`] via `#[serde(flatten)]`. Exactly
/// one of the three keys must be present per rule; serde rejects
/// duplicates at deserialise time.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    /// Redact matching entities. The carried map names operators
    /// per modality; modalities the rule didn't cover fall through
    /// to the deployment-wide default.
    Redact(ModalityRedactions),
    /// Suppress matching entities — treat as a false positive and
    /// skip redaction. Audit entry records the suppression.
    Suppress(SuppressAction),
    /// Flag matching entities for human review. The entity is
    /// left untouched; the audit entry carries the severity hint
    /// for downstream tooling.
    Audit(AuditAction),
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
pub struct PolicyRule {
    /// Author-supplied rule name. Must be unique within the
    /// owning policy; audit entries reference this string verbatim.
    #[schemars(with = "String")]
    pub name: HipStr<'static>,
    /// Which entities this rule applies to. Authored as
    /// `match = { ... }` for parity with the policy-DSL phrasing.
    #[serde(rename = "match")]
    pub selector: EntitySelector,
    /// What this rule does when it matches. Flattened so the
    /// variant key (`redact` / `suppress` / `audit`) sits at the
    /// rule's top level — the shape author-facing TOML reads most
    /// naturally.
    #[serde(flatten)]
    pub action: Action,
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
