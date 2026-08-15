//! Authored vocabulary for redaction governance: policies, the
//! rules inside them, the predicates that gate those rules, and
//! the operator specs the rules dispatch to.

mod origin;
mod predicate;
mod rule;
mod scope;

use elide_core::entity::{Label, LabelRef};
use hipstr::HipStr;
pub use predicate::Predicate;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub use self::origin::TemplateOrigin;
pub use self::rule::{LabelEntry, PolicyRule, RuleDispatch};
pub use self::scope::LabelScope;
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
    /// What this policy detects: one or more named, attributed
    /// label sets.
    ///
    /// The union of every scope, plus [`custom`], is the policy's
    /// recognition vocabulary. A label no scope names is never
    /// detected, so no rule can fire on it and the policy is inert
    /// with respect to it.
    ///
    /// Detecting more than the rules act on is deliberate: scope a
    /// whole regulatory category, write rules for the labels
    /// needing special treatment, and let [`fallback`] sweep the
    /// rest.
    ///
    /// [`custom`]: Self::custom
    /// [`fallback`]: Self::fallback
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scopes: Vec<LabelScope>,
    /// Caller-authored label schemas this policy introduces.
    ///
    /// Only for labels elide does not ship. These join the
    /// recognition vocabulary alongside [`scopes`], and a rule may
    /// target them the same way.
    ///
    /// [`scopes`]: Self::scopes
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub custom: Vec<Label>,
    /// Ordered rules. First match wins within this policy.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rules: Vec<PolicyRule>,
    /// Per-policy catch-all, applied to any detected entity in
    /// the policy's vocabulary that no rule claimed. Fires when no
    /// rule in this policy matched. Presence halts the chain; absence falls through
    /// to the next policy. [`Option`] enforces "at most one
    /// fallback per policy" at the type level.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback: Option<ModalityRedactions>,
}

impl PolicyDefinition {
    /// Every label this policy detects: the union of its
    /// [`scopes`] and its [`custom`] schemas.
    ///
    /// The engine unions this across all submitted policies into
    /// the per-request `LabelCatalog` that drives recognizer
    /// dispatch, and applies it again at match time so one policy
    /// cannot act on an entity another policy pulled in.
    ///
    /// Order follows declaration order, scopes first, and a label
    /// named twice appears once.
    ///
    /// [`custom`]: Self::custom
    /// [`scopes`]: Self::scopes
    #[must_use]
    pub fn label_scope(&self) -> Vec<LabelRef> {
        let mut scope: Vec<LabelRef> = Vec::new();
        let mut push = |label: LabelRef| {
            if !scope.contains(&label) {
                scope.push(label);
            }
        };
        for declared in &self.scopes {
            for label in &declared.labels {
                push(label.clone());
            }
        }
        for label in &self.custom {
            push(label.to_ref());
        }
        scope
    }

    /// The labels of the scope named `name`, if this policy
    /// declares one.
    ///
    /// Scopes are policy-local: a rule can only name a scope its
    /// own policy declared.
    #[must_use]
    pub fn scope_named(&self, name: &str) -> Option<&[LabelRef]> {
        self.scopes
            .iter()
            .find(|s| s.name == name)
            .map(|s| s.labels.as_slice())
    }
}
