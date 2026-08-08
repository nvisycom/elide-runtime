//! Attach a [`PolicyRule`] / `fallback` onto an elide [`Anonymizer`],
//! stamping the engine-internal attribution from policy + rule
//! UUIDs.
//!
//! The wire carries one composable [`Predicate`] per rule.
//! [`attach`] pattern-matches that predicate against three
//! degenerate shapes to keep elide's fast paths:
//!
//! - [`Predicate::LabelOneOf`] with a single label →
//!   [`Rule::label`] (audit records the matched label).
//! - [`Predicate::TagOneOf`] with a single tag →
//!   [`Rule::tag`] (audit records the matched tag).
//! - Anything else → [`Rule::predicate`] with a closure over the
//!   full [`MatchContext`] (audit records `Predicate`).
//!
//! [`Rule::label`]: elide::redaction::Rule::label
//! [`Rule::tag`]: elide::redaction::Rule::tag
//! [`Rule::predicate`]: elide::redaction::Rule::predicate
//! [`MatchContext`]: elide::redaction::MatchContext
//!
//! Attribution always sits in elide's [`Attribution`] slot:
//! `name` from the policy's UUID, `description` from the rule's
//! UUID (or omitted for the policy fallback).
//!
//! [`Anonymizer`]: elide::redaction::Anonymizer
//! [`PolicyRule`]: nvisy_schema::policy::PolicyRule
//! [`Predicate`]: nvisy_schema::policy::predicate::Predicate
//! [`Attribution`]: elide_core::entity::provenance::Attribution

use elide::redaction::{Anonymizer, MatchContext, Rule};
use elide_core::entity::provenance::Attribution;
use elide_core::modality::Modality;
use elide_core::operator::Operator;
use nvisy_schema::policy::predicate::Predicate;
use nvisy_schema::policy::{PolicyDefinition, PolicyRule};
use uuid::Uuid;

use crate::analyzer::GROUP_TAG_PREFIX;

/// Build an [`Attribution`] for a concrete rule that fired.
///
/// `name` carries the policy's stable UUID (elide's
/// [`Attribution::name`] is the "policy authority" slot) and
/// `description` carries the rule's UUID so an audit can trace a
/// redaction back to the exact rule inside the policy.
pub(super) fn rule_attribution(policy: &PolicyDefinition, rule: &PolicyRule) -> Attribution {
    Attribution::new(policy.id.to_string()).with_description(rule.id().to_string())
}

/// Build an [`Attribution`] for a policy's `fallback`.
///
/// No `description` — the fallback is the policy's catch-all,
/// there is no per-rule id to record.
pub(super) fn fallback_attribution(policy: &PolicyDefinition) -> Attribution {
    Attribution::new(policy.id.to_string())
}

/// Build an [`Attribution`] for a reviewer override.
///
/// Uses the sentinel `name = "override"` so audits can
/// distinguish override-driven redactions from policy-driven
/// ones; `description` carries the overridden entity's id.
pub(super) fn override_attribution(entity_id: Uuid) -> Attribution {
    Attribution::new("override").with_description(entity_id.to_string())
}

/// Attach an `operator` to `anonymizer` for the single entity
/// identified by `entity_id`. Used by the apply pipeline to give
/// reviewer overrides higher precedence than any policy-driven
/// rule.
pub(super) fn attach_override<M, O>(
    anonymizer: Anonymizer<M>,
    entity_id: Uuid,
    operator: O,
) -> Anonymizer<M>
where
    M: Modality,
    O: Operator<M> + 'static,
{
    anonymizer.with(
        Rule::predicate(
            move |cx: &MatchContext<'_, M>| cx.entity.id == entity_id,
            operator,
        )
        .because(override_attribution(entity_id)),
    )
}

/// Attach `operator` to `anonymizer` for the rule's
/// [`Predicate`], stamping `attribution` so every redaction it
/// drives carries the policy's identity on its provenance event.
///
/// Pattern-matches the predicate against degenerate single-label
/// / single-tag shapes to keep elide's indexed fast paths. Any
/// composite predicate falls through to [`Rule::predicate`] with
/// a closure over the full [`MatchContext`].
pub(super) fn attach<M, O>(
    anonymizer: Anonymizer<M>,
    predicate: &Predicate,
    operator: O,
    attribution: Attribution,
) -> Anonymizer<M>
where
    M: Modality,
    O: Operator<M> + 'static,
{
    let rule = match predicate {
        Predicate::LabelOneOf { labels } if labels.len() == 1 => {
            Rule::label(labels[0].clone(), operator)
        }
        Predicate::TagOneOf { tags } if tags.len() == 1 => {
            Rule::tag(tags[0].clone(), operator)
        }
        Predicate::LabelInGroup { group } => {
            // Groups compile to a synthetic `group:<name>` tag on
            // every listed label (see `analyzer::catalog`), so a
            // group predicate takes the same fast path as any
            // single-tag `TagOneOf`.
            Rule::tag(format!("{GROUP_TAG_PREFIX}{group}"), operator)
        }
        other => Rule::predicate(compile_predicate::<M>(other.clone()), operator),
    };
    anonymizer.with(rule.because(attribution))
}

/// Compile a [`Predicate`] tree into a closure consumed by
/// [`Rule::predicate`]. The closure receives the full
/// [`MatchContext`], so [`Predicate::TagOneOf`] resolves against
/// the per-anonymizer [`LabelCatalog`] even inside composites
/// ([`All`] / [`Any`] / [`Not`]).
///
/// [`All`]: Predicate::All
/// [`Any`]: Predicate::Any
/// [`Not`]: Predicate::Not
pub(super) fn compile_predicate<M>(
    predicate: Predicate,
) -> impl Fn(&MatchContext<'_, M>) -> bool + Send + Sync + 'static
where
    M: Modality,
{
    move |cx| eval(&predicate, cx)
}

fn eval<M: Modality>(predicate: &Predicate, cx: &MatchContext<'_, M>) -> bool {
    match predicate {
        Predicate::Confidence { min } => f32::from(cx.entity.confidence) >= f32::from(*min),
        Predicate::LabelOneOf { labels } => labels.iter().any(|l| l == &cx.entity.label),
        Predicate::TagOneOf { tags } => cx
            .catalog
            .get(&cx.entity.label)
            .is_some_and(|label| tags.iter().any(|tag| label.has_tag(tag.as_str()))),
        Predicate::LabelInGroup { group } => {
            let synthetic_tag = format!("{GROUP_TAG_PREFIX}{group}");
            cx.catalog
                .get(&cx.entity.label)
                .is_some_and(|label| label.has_tag(&synthetic_tag))
        }
        Predicate::CoRef { coref } => cx
            .entity
            .coref
            .as_ref()
            .is_some_and(|c| c.as_str() == coref.as_str()),
        Predicate::All { all } => all.iter().all(|p| eval(p, cx)),
        Predicate::Any { any } => any.iter().any(|p| eval(p, cx)),
        Predicate::Not { not } => !eval(not, cx),
    }
}
