//! Authored vocabulary for redaction governance: policies, the
//! rules inside them, the predicates that gate those rules, and
//! the operator specs the rules dispatch to.

mod label;
mod origin;
mod predicate;
mod rule;

use hipstr::HipStr;
pub use predicate::Predicate;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub use self::label::{LabelGroup, Labels};
pub use self::origin::TemplateOrigin;
pub use self::rule::{LabelEntry, PolicyRule, RuleDispatch};
use crate::redaction::ModalityRedactions;

/// A named governance policy.
///
/// Identity is the UUID; `name` is display-only.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PolicyDefinition {
    /// Stable identifier. UUIDv7 recommended (time-ordered);
    /// customer-supplied so re-submissions carry the same id.
    pub id: Uuid,
    /// Human-readable name. Display-only. Does not key anything.
    ///
    /// Names the policy in a redaction event's [`Attribution`]
    /// when a rule that fired carried no [`AttributionKind::Cited`]
    /// attribution to render.
    ///
    /// [`Attribution`]: elide_core::entity::audit::Attribution
    /// [`AttributionKind::Cited`]: elide_core::entity::audit::AttributionKind::Cited
    #[schemars(with = "String")]
    pub name: HipStr<'static>,
    /// Optional description for reviewers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<String>")]
    pub description: Option<HipStr<'static>>,
    /// The shipped template this policy was built from, when it
    /// was.
    ///
    /// Provenance, not fidelity: callers are expected to mutate a
    /// template's policy before submitting, so this records where
    /// the policy came from and says nothing about whether it
    /// still matches. `None` means hand-authored.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub template: Option<TemplateOrigin>,
    /// Vocabulary the policy operates over: builtins picked by
    /// name plus caller-authored custom label schemas. Engine
    /// unions every submitted policy's `labels` into a per-request
    /// [`LabelCatalog`] used to drive recognizer dispatch and
    /// tag-based [`Predicate::TagOneOf`] matching.
    ///
    /// [`LabelCatalog`]: elide_core::entity::LabelCatalog
    /// [`Predicate::TagOneOf`]: crate::Predicate::TagOneOf
    #[serde(default, skip_serializing_if = "Labels::is_empty")]
    pub labels: Labels,
    /// Named clusters of [`LabelRef`]s this policy's rules may
    /// reference by name via [`Predicate::LabelInGroup`]. Scoped
    /// to this policy: a rule can only name a group its own
    /// policy declared; unknown references error at request
    /// validation. Two policies that both declare `hipaa_18` with
    /// different labelsets stay independent.
    ///
    /// [`LabelRef`]: elide_core::entity::LabelRef
    /// [`Predicate::LabelInGroup`]: crate::Predicate::LabelInGroup
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub groups: Vec<LabelGroup>,
    /// Ordered rules. First match wins within this policy.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rules: Vec<PolicyRule>,
    /// Per-policy catch-all. Fires when no rule in this policy
    /// matched. Presence halts the chain; absence falls through
    /// to the next policy. [`Option`] enforces "at most one
    /// fallback per policy" at the type level.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback: Option<ModalityRedactions>,
}
