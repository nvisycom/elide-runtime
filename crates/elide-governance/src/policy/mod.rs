//! Authored vocabulary for redaction governance: policies, the
//! rules inside them, the predicates that gate those rules, and
//! the operator specs the rules dispatch to.

mod label;
mod origin;
mod predicate;
mod rule;

use elide_core::entity::{Label, LabelRef};
use hipstr::HipStr;
pub use predicate::Predicate;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub use self::label::LabelGroup;
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
    /// Caller-authored label schemas this policy introduces.
    ///
    /// Only for labels elide does not ship. Builtins are never
    /// declared: a rule that targets one already names it, and
    /// [`label_scope`] derives the policy's vocabulary from the
    /// rules rather than from a second hand-maintained list that
    /// could drift out of step with them.
    ///
    /// [`label_scope`]: Self::label_scope
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub custom: Vec<Label>,
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

impl PolicyDefinition {
    /// Every label this policy operates over, derived from what
    /// its rules actually target.
    ///
    /// Unions three sources:
    ///
    /// - every [`LabelGroup`]'s labels, since a rule referencing
    ///   the group targets all of them;
    /// - every [`RuleDispatch::Table`] entry's label;
    /// - every [`Predicate::LabelOneOf`] leaf's labels, at any
    ///   depth inside an `All` / `Any` / `Not` tree;
    /// - plus [`custom`](Self::custom), whose schemas exist
    ///   nowhere else.
    ///
    /// The engine unions this across every submitted policy into
    /// the per-request `LabelCatalog` that drives recognizer
    /// dispatch, and uses it again at match time so one policy
    /// cannot act on an entity another policy pulled in.
    ///
    /// Deriving rather than declaring means the vocabulary cannot
    /// drift from the rules: a label the rules target is always
    /// detected, and a label nothing targets is never detected
    /// only to pass through unredacted.
    ///
    /// [`Predicate::LabelOneOf`]: crate::Predicate::LabelOneOf
    #[must_use]
    pub fn label_scope(&self) -> Vec<LabelRef> {
        let mut scope: Vec<LabelRef> = Vec::new();
        let mut push = |label: &LabelRef| {
            if !scope.contains(label) {
                scope.push(label.clone());
            }
        };

        for group in &self.groups {
            for label in &group.labels {
                push(label);
            }
        }
        for rule in &self.rules {
            match &rule.dispatch {
                RuleDispatch::Table { operators } => {
                    for entry in operators {
                        push(&entry.label);
                    }
                }
                RuleDispatch::Predicated { predicate, .. } => {
                    collect_predicate_labels(predicate, &mut push);
                }
            }
        }
        for label in &self.custom {
            push(&label.to_ref());
        }
        scope
    }
}

/// Walk a [`Predicate`] tree, handing every [`LabelOneOf`] leaf's
/// labels to `push`.
///
/// The other leaves name no label: `TagOneOf`, `Confidence`, and
/// `CoRef` filter entities that recognition already produced, so
/// they contribute nothing to what should be detected. A policy
/// built only from those derives an empty scope and is inert.
///
/// [`LabelOneOf`]: crate::Predicate::LabelOneOf
fn collect_predicate_labels(predicate: &Predicate, push: &mut impl FnMut(&LabelRef)) {
    match predicate {
        Predicate::LabelOneOf { labels } => {
            for label in labels {
                push(label);
            }
        }
        Predicate::All { all } => {
            for inner in all {
                collect_predicate_labels(inner, push);
            }
        }
        Predicate::Any { any } => {
            for inner in any {
                collect_predicate_labels(inner, push);
            }
        }
        Predicate::Not { not } => collect_predicate_labels(not, push),
        Predicate::TagOneOf { .. }
        | Predicate::Confidence { .. }
        | Predicate::LabelInGroup { .. }
        | Predicate::CoRef { .. } => {}
    }
}

#[cfg(test)]
mod tests {
    use elide_core::entity::LabelRef;

    use super::*;
    use crate::redaction::{ModalityRedactions, TextRedaction};

    fn policy(groups: Vec<LabelGroup>, rules: Vec<PolicyRule>) -> PolicyDefinition {
        PolicyDefinition {
            id: Uuid::nil(),
            name: HipStr::borrowed("test"),
            description: None,
            template: None,
            custom: Vec::new(),
            groups,
            rules,
            fallback: None,
        }
    }

    fn group(name: &'static str, labels: &[&'static str]) -> LabelGroup {
        LabelGroup {
            name: HipStr::borrowed(name),
            description: None,
            attribution: None,
            labels: labels.iter().map(|l| LabelRef::from_static(l)).collect(),
        }
    }

    fn predicated(predicate: Predicate) -> PolicyRule {
        PolicyRule {
            id: Uuid::nil(),
            name: HipStr::borrowed("r"),
            description: None,
            attribution: None,
            dispatch: RuleDispatch::Predicated {
                predicate,
                action: Box::new(ModalityRedactions::text(TextRedaction::Erase)),
            },
        }
    }

    fn scope_of(policy: &PolicyDefinition) -> Vec<String> {
        let mut v: Vec<String> = policy
            .label_scope()
            .iter()
            .map(|l| l.as_str().to_owned())
            .collect();
        v.sort();
        v
    }

    #[test]
    fn groups_contribute_their_labels() {
        let p = policy(
            vec![group("g", &["email_address", "phone_number"])],
            Vec::new(),
        );
        assert_eq!(scope_of(&p), ["email_address", "phone_number"]);
    }

    #[test]
    fn table_entries_contribute_their_labels() {
        let rule = PolicyRule {
            id: Uuid::nil(),
            name: HipStr::borrowed("t"),
            description: None,
            attribution: None,
            dispatch: RuleDispatch::Table {
                operators: vec![LabelEntry {
                    label: LabelRef::from_static("age"),
                    action: ModalityRedactions::text(TextRedaction::Erase),
                }],
            },
        };
        assert_eq!(scope_of(&policy(Vec::new(), vec![rule])), ["age"]);
    }

    #[test]
    fn label_one_of_contributes_at_any_predicate_depth() {
        // Nested inside All/Not so a shallow walk would miss it.
        let p = policy(
            Vec::new(),
            vec![predicated(Predicate::All {
                all: vec![Predicate::Not {
                    not: Box::new(Predicate::LabelOneOf {
                        labels: vec![LabelRef::from_static("iban")],
                    }),
                }],
            })],
        );
        assert_eq!(scope_of(&p), ["iban"]);
    }

    #[test]
    fn label_less_predicates_contribute_nothing() {
        // Tag, confidence, and coref filter entities recognition
        // already produced; they cannot say what to detect. Such a
        // policy derives an empty scope and is inert rather than an
        // error.
        for predicate in [
            Predicate::TagOneOf {
                tags: vec!["financial".to_owned()],
            },
            Predicate::Confidence {
                min: 0.8_f32.try_into().expect("0.8 is a valid threshold"),
            },
            Predicate::CoRef {
                coref: "subject-1".to_owned(),
            },
        ] {
            let p = policy(Vec::new(), vec![predicated(predicate)]);
            assert!(p.label_scope().is_empty());
        }
    }

    #[test]
    fn a_label_reached_twice_appears_once() {
        let p = policy(
            vec![
                group("a", &["email_address"]),
                group("b", &["email_address"]),
            ],
            vec![predicated(Predicate::LabelOneOf {
                labels: vec![LabelRef::from_static("email_address")],
            })],
        );
        assert_eq!(scope_of(&p), ["email_address"]);
    }
}
