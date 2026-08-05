//! Attach a [`PolicyRule`] / `fallback` onto an elide [`Anonymizer`],
//! stamping the engine-internal attribution from policy + rule
//! UUIDs.
//!
//! The wire now carries one composable [`Predicate`] per rule.
//! [`attach`] pattern-matches that predicate against three
//! degenerate shapes to keep elide's fast paths:
//!
//! - [`Predicate::LabelOneOf`] with a single label →
//!   [`Anonymizer::with_label`] (audit records the matched
//!   label).
//! - [`Predicate::TagOneOf`] with a single tag →
//!   [`Anonymizer::with_tag`] (audit records the matched tag).
//! - Anything else → [`Anonymizer::with_catalog_predicate`]
//!   (audit records `Predicate`).
//!
//! Attribution always sits in elide's [`Attribution`] slot:
//! `policy_id` from the policy's UUID, `reason` from the rule's
//! UUID (or `None` for the policy fallback).
//!
//! [`Anonymizer`]: elide::redaction::Anonymizer
//! [`PolicyRule`]: nvisy_schema::policy::PolicyRule
//! [`Predicate`]: nvisy_schema::policy::predicate::Predicate
//! [`Attribution`]: elide_core::entity::provenance::Attribution

use elide::redaction::Anonymizer;
use elide_core::entity::provenance::Attribution;
use elide_core::entity::{Entity, LabelCatalog};
use elide_core::modality::Modality;
use elide_core::operator::Operator;
use nvisy_schema::policy::predicate::Predicate;
use nvisy_schema::policy::{PolicyDefinition, PolicyRule};
use uuid::Uuid;

/// Build an [`Attribution`] for a concrete rule that fired.
pub(super) fn rule_attribution(policy: &PolicyDefinition, rule: &PolicyRule) -> Attribution {
    Attribution {
        policy_id: policy.id.to_string().into(),
        reason: Some(rule.id.to_string().into()),
    }
}

/// Build an [`Attribution`] for a policy's `fallback`.
pub(super) fn fallback_attribution(policy: &PolicyDefinition) -> Attribution {
    Attribution {
        policy_id: policy.id.to_string().into(),
        reason: None,
    }
}

/// Build an [`Attribution`] for a reviewer override. Uses the
/// sentinel `policy_id = "override"` so audits can distinguish
/// override-driven redactions from policy-driven ones; `reason`
/// carries the overridden entity's id.
pub(super) fn override_attribution(entity_id: Uuid) -> Attribution {
    Attribution {
        policy_id: "override".into(),
        reason: Some(entity_id.to_string().into()),
    }
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
    let attribution = override_attribution(entity_id);
    anonymizer
        .with_predicate(move |entity: &Entity<M>| entity.id == entity_id, operator)
        .because(attribution)
}

/// Attach `operator` to `anonymizer` for the rule's
/// [`Predicate`], stamping `attribution` so every redaction it
/// drives carries `policy_id` + `reason` on its provenance event.
///
/// Pattern-matches the predicate against degenerate single-label
/// / single-tag shapes to keep elide's indexed fast paths. Any
/// composite predicate falls through to
/// [`Anonymizer::with_catalog_predicate`].
pub(super) fn attach<M, O>(
    anonymizer: Anonymizer<M>,
    predicate: &Predicate,
    operator: O,
    attribution: Attribution,
) -> Anonymizer<M>
where
    M: Modality,
    O: Operator<M> + Clone + 'static,
{
    let anonymizer = match predicate {
        Predicate::LabelOneOf { labels } if labels.len() == 1 => {
            anonymizer.with_label(labels[0].clone(), operator)
        }
        Predicate::TagOneOf { tags } if tags.len() == 1 => {
            anonymizer.with_tag(tags[0].clone(), operator)
        }
        other => anonymizer.with_catalog_predicate(compile_predicate::<M>(other.clone()), operator),
    };
    anonymizer.because(attribution)
}

/// Compile a [`Predicate`] tree into a closure consumed by
/// [`Anonymizer::with_catalog_predicate`]. The closure receives
/// the per-anonymizer [`LabelCatalog`] alongside the entity so
/// [`Predicate::TagOneOf`] resolves correctly even inside
/// composites ([`All`] / [`Any`] / [`Not`]).
///
/// [`Anonymizer::with_catalog_predicate`]: elide::redaction::Anonymizer::with_catalog_predicate
/// [`All`]: Predicate::All
/// [`Any`]: Predicate::Any
/// [`Not`]: Predicate::Not
pub(super) fn compile_predicate<M>(
    predicate: Predicate,
) -> impl Fn(&Entity<M>, &LabelCatalog) -> bool + Send + Sync + 'static
where
    M: Modality,
{
    move |entity, catalog| eval(&predicate, entity, catalog)
}

fn eval<M: Modality>(predicate: &Predicate, entity: &Entity<M>, catalog: &LabelCatalog) -> bool {
    match predicate {
        Predicate::Confidence { min } => f32::from(entity.confidence) >= f32::from(*min),
        Predicate::LabelOneOf { labels } => labels
            .iter()
            .any(|l| l == &entity.label),
        Predicate::TagOneOf { tags } => catalog
            .get(&entity.label)
            .is_some_and(|label| tags.iter().any(|tag| label.has_tag(tag.as_str()))),
        Predicate::CoRef { coref } => entity
            .coref
            .as_ref()
            .is_some_and(|c| c.as_str() == coref.as_str()),
        Predicate::All { all } => all.iter().all(|p| eval(p, entity, catalog)),
        Predicate::Any { any } => any.iter().any(|p| eval(p, entity, catalog)),
        Predicate::Not { not } => !eval(not, entity, catalog),
    }
}
