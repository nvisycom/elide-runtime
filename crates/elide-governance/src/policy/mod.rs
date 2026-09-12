//! Authored vocabulary for redaction governance: policies, the
//! rules inside them, the predicates that gate those rules, and
//! the operator specs the rules dispatch to.

mod origin;
mod predicate;
mod rule;
mod scope;

use elide_core::entity::LabelRef;
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
pub struct Policy {
    /// Stable identifier. UUIDv7 recommended (time-ordered);
    /// customer-supplied so re-submissions carry the same id.
    pub id: Uuid,
    /// Human-readable name. Display-only. Does not key anything.
    ///
    /// Names the policy in a redaction event's [`Attribution`]
    /// when a rule that fired carried no [`Attribution::Cited`]
    /// attribution to render.
    ///
    /// [`Attribution`]: elide_core::entity::audit::Attribution
    /// [`Attribution::Cited`]: elide_core::entity::audit::Attribution::Cited
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
    /// The union of every scope is what this policy acts on. A
    /// label no scope names is never detected, so no rule can fire
    /// on it and the policy is inert with respect to it — whether
    /// the label is one elide ships or one the request introduced.
    ///
    /// Detecting more than the rules act on is deliberate: scope a
    /// whole regulatory category, write rules for the labels
    /// needing special treatment, and let [`fallback`] sweep the
    /// rest.
    ///
    /// [`fallback`]: Self::fallback
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scopes: Vec<LabelScope>,
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

/// A policy with a fresh identity and nothing else set: no scopes,
/// no custom labels or matchers, no rules, no fallback.
///
/// Hand-written rather than derived so `id` is a fresh UUIDv7
/// rather than nil — identity is the UUID, and a derived default
/// would hand every policy the same one, which two policies in a
/// request would silently collide on.
///
/// For construction where most fields are absent, chiefly tests
/// and callers building a policy up field by field:
///
/// ```
/// # use elide_governance::policy::Policy;
/// # use elide_governance::redaction::{ModalityRedactions, TextRedaction};
/// let sweep = Policy {
///     name: "sweep".into(),
///     fallback: Some(ModalityRedactions::textual(TextRedaction::Erase)),
///     ..Policy::default()
/// };
/// ```
impl Default for Policy {
    fn default() -> Self {
        Self {
            id: Uuid::now_v7(),
            name: HipStr::default(),
            description: None,
            template: None,
            scopes: Vec::new(),
            rules: Vec::new(),
            fallback: None,
        }
    }
}

impl Policy {
    /// Every label this policy detects: the union of its
    /// [`scopes`].
    ///
    /// A label the request introduces reaches a policy the same
    /// way a shipped one does — by being scoped. The vocabulary is
    /// request-wide; a policy's scopes say which of it this policy
    /// acts on.
    ///
    /// The engine unions this across all submitted policies into
    /// the per-request `LabelCatalog` that drives recognizer
    /// dispatch, and applies it again at match time so one policy
    /// cannot act on an entity another policy pulled in.
    ///
    /// Order follows declaration order, and a label named twice
    /// appears once.
    ///
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
        scope
    }

    /// Whether this policy carries operators it can never run.
    ///
    /// Scopes flatten into a request-wide label catalog for
    /// analysis, but each policy's own scope gates its redaction:
    /// a rule matches an entity only when the scope contains its
    /// label. A policy declaring `LabelScope::new("x", vec![])`
    /// alongside an operator therefore contributes nothing to
    /// detection and matches nothing at redaction, while any
    /// *other* policy's labels still reach it as entities it
    /// cannot act on — detected, and silently left in place.
    ///
    /// `false` for a policy that names no operators, which redacts
    /// nothing by design, and for one whose [`scopes`] are absent
    /// rather than empty, which says nothing about coverage.
    ///
    /// [`scopes`]: Self::scopes
    #[must_use]
    pub fn scopes_nothing_it_redacts(&self) -> bool {
        let redacts = self.fallback.is_some() || !self.rules.is_empty();
        redacts && !self.scopes.is_empty() && self.label_scope().is_empty()
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
