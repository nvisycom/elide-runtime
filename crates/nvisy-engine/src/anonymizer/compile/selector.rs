//! Attach a [`PolicyRule`] / `fallback` onto an elide [`Anonymizer`],
//! stamping the engine-internal attribution from policy + rule
//! UUIDs and enforcing per-policy label + group scoping.
//!
//! Every predicate compiles into a closure passed to
//! [`Rule::predicate`]. A rule declared inside policy A can only
//! fire on entities whose label is in `policy.labels` (its own
//! declared vocabulary), and any [`Predicate::LabelInGroup`] leaf
//! resolves against `policy.groups`, not the shared label catalog.
//! There is no request-shared string namespace (like a synthetic
//! `group:*` tag) that another policy's [`Predicate::TagOneOf`]
//! could exploit to bypass scoping.
//!
//! The uniform closure path costs a per-entity closure call over
//! elide's indexed [`Rule::label`] / [`Rule::tag`] fast paths, but
//! those fast paths cannot enforce per-policy label scoping and
//! would let scoping become an authoring convention rather than a
//! runtime guarantee. Correctness over speed.
//!
//! [`Anonymizer`]: elide::redaction::Anonymizer
//! [`MatchContext`]: elide::redaction::MatchContext
//! [`PolicyRule`]: nvisy_schema::policy::PolicyRule
//! [`Predicate`]: nvisy_schema::policy::predicate::Predicate
//! [`Predicate::LabelInGroup`]: nvisy_schema::policy::predicate::Predicate::LabelInGroup
//! [`Predicate::TagOneOf`]: nvisy_schema::policy::predicate::Predicate::TagOneOf
//! [`Rule::label`]: elide::redaction::Rule::label
//! [`Rule::predicate`]: elide::redaction::Rule::predicate
//! [`Rule::tag`]: elide::redaction::Rule::tag
//!
//! Attribution always sits in elide's [`Attribution`] slot:
//! `name` from the policy's UUID, `description` from the rule's
//! UUID (or omitted for the policy fallback).
//!
//! [`Attribution`]: elide_core::entity::provenance::Attribution

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use elide::redaction::{Anonymizer, MatchContext, Rule};
use elide_core::entity::LabelRef;
use elide_core::entity::provenance::Attribution;
use elide_core::modality::Modality;
use elide_core::operator::Operator;
use nvisy_schema::policy::predicate::Predicate;
use nvisy_schema::policy::{LabelGroup, PolicyDefinition, PolicyRule};
use uuid::Uuid;

use crate::analyzer::policy_label_scope;

/// Per-policy scoping context threaded into every predicate the
/// selector compiles. Built once per policy at attach time; every
/// rule inside that policy shares the same reference-counted copy.
///
/// - `label_scope` — the [`LabelRef`]s the policy declares in its
///   [`labels`] block. Every predicate filters by whether the
///   candidate entity's label is in this set, so a policy that
///   lists only `email_address` never fires on a `phone_number`
///   another policy pulled into the request's recognition pass.
/// - `groups` — resolved group name → labelset lookup, materialised
///   from the policy's [`groups`] block once. A rule can only
///   name a group its own policy declared (validated separately
///   in `pipeline::orchestrator::validate_group_references`).
///
/// [`labels`]: nvisy_schema::policy::PolicyDefinition::labels
/// [`groups`]: nvisy_schema::policy::PolicyDefinition::groups
#[derive(Clone)]
pub(in crate::anonymizer) struct PolicyContext {
    /// The enclosing policy's UUID. Threaded into per-policy
    /// operator infrastructure lookups: `HmacHash`/`Encrypt`
    /// resolve their [`KeyProvider`] per-policy first, and
    /// [`Pseudonymize`] draws from a per-policy vault.
    ///
    /// [`KeyProvider`]: elide::redaction::operators::KeyProvider
    /// [`Pseudonymize`]: elide::redaction::operators::Pseudonymize
    pub(in crate::anonymizer) policy_id: Uuid,
    label_scope: Arc<HashSet<LabelRef>>,
    groups: Arc<HashMap<String, HashSet<LabelRef>>>,
}

impl PolicyContext {
    /// Materialise a policy's scoping context from its
    /// [`labels`] and [`groups`] blocks.
    ///
    /// [`labels`]: nvisy_schema::policy::PolicyDefinition::labels
    /// [`groups`]: nvisy_schema::policy::PolicyDefinition::groups
    pub(in crate::anonymizer) fn from_policy(policy: &PolicyDefinition) -> Self {
        let label_scope: HashSet<LabelRef> = policy_label_scope(policy).into_iter().collect();
        let groups: HashMap<String, HashSet<LabelRef>> =
            policy.groups.iter().map(group_lookup_entry).collect();
        Self {
            policy_id: policy.id,
            label_scope: Arc::new(label_scope),
            groups: Arc::new(groups),
        }
    }

    /// Whether the enclosing policy declared vocabulary for
    /// `label`. The gate every predicate — including the policy
    /// fallback — passes through before any per-predicate logic
    /// evaluates.
    pub(in crate::anonymizer) fn label_scope_contains(&self, label: &LabelRef) -> bool {
        self.label_scope.contains(label)
    }
}

fn group_lookup_entry(group: &LabelGroup) -> (String, HashSet<LabelRef>) {
    let labels: HashSet<LabelRef> = group.labels.iter().cloned().collect();
    (group.name.to_string(), labels)
}

/// Build an [`Attribution`] for a concrete rule that fired.
///
/// `name` carries the policy's stable UUID (elide's
/// [`Attribution::name`] is the "policy authority" slot) and
/// `description` carries the rule's UUID so an audit can trace a
/// redaction back to the exact rule inside the policy.
pub(super) fn rule_attribution(policy: &PolicyDefinition, rule: &PolicyRule) -> Attribution {
    Attribution::new(policy.id.to_string()).with_description(rule.id.to_string())
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
/// `name` carries the policy UUID the reviewer exercised
/// authority under — same slot as any policy-driven rule so an
/// auditor grepping by policy id sees override-driven redactions
/// too. `description` combines a fixed `"override:"` prefix with
/// the overridden entity's id so audits can still distinguish
/// override events from rule events at a glance.
pub(super) fn override_attribution(entity_id: Uuid, policy_id: Uuid) -> Attribution {
    Attribution::new(policy_id.to_string()).with_description(format!("override:{entity_id}"))
}

/// Attach an `operator` to `anonymizer` for the single entity
/// identified by `entity_id`. Used by the apply pipeline to give
/// reviewer overrides higher precedence than any policy-driven
/// rule. `policy_id` names the overriding policy — attribution
/// stamps it, and per-policy operator infrastructure (pseudonym
/// vault, `KeyProvider`) resolves against it.
pub(super) fn attach_override<M, O>(
    anonymizer: Anonymizer<M>,
    entity_id: Uuid,
    operator: O,
    policy_id: Uuid,
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
        .because(override_attribution(entity_id, policy_id)),
    )
}

/// Attach `operator` to `anonymizer` for the rule's
/// [`Predicate`], stamping `attribution` so every redaction it
/// drives carries the policy's identity on its provenance event.
///
/// Compiles the predicate into a closure that filters by
/// `context.label_scope` before evaluating the tree; a rule
/// cannot fire on labels its policy did not declare.
///
/// [`Predicate`]: nvisy_schema::policy::predicate::Predicate
pub(super) fn attach<M, O>(
    anonymizer: Anonymizer<M>,
    predicate: &Predicate,
    operator: O,
    attribution: Attribution,
    context: PolicyContext,
) -> Anonymizer<M>
where
    M: Modality,
    O: Operator<M> + 'static,
{
    let closure = compile_predicate::<M>(predicate.clone(), context);
    anonymizer.with(Rule::predicate(closure, operator).because(attribution))
}

/// Compile a [`Predicate`] tree into a closure consumed by
/// [`Rule::predicate`]. The closure filters every candidate
/// entity through the enclosing policy's [`label_scope`] before
/// evaluating the tree; any [`Predicate::LabelInGroup`] leaf
/// resolves against the policy's own materialised group table.
///
/// [`label_scope`]: PolicyContext
/// [`Rule::predicate`]: elide::redaction::Rule::predicate
pub(super) fn compile_predicate<M>(
    predicate: Predicate,
    context: PolicyContext,
) -> impl Fn(&MatchContext<'_, M>) -> bool + Send + Sync + 'static
where
    M: Modality,
{
    move |cx| context.label_scope.contains(&cx.entity.label) && eval(&predicate, cx, &context)
}

fn eval<M: Modality>(
    predicate: &Predicate,
    cx: &MatchContext<'_, M>,
    context: &PolicyContext,
) -> bool {
    match predicate {
        Predicate::Confidence { min } => f32::from(cx.entity.confidence) >= f32::from(*min),
        Predicate::LabelOneOf { labels } => labels.iter().any(|l| l == &cx.entity.label),
        Predicate::TagOneOf { tags } => cx
            .catalog
            .get(&cx.entity.label)
            .is_some_and(|label| tags.iter().any(|tag| label.has_tag(tag.as_str()))),
        Predicate::LabelInGroup { group } => context
            .groups
            .get(group.as_str())
            .is_some_and(|labels| labels.contains(&cx.entity.label)),
        Predicate::CoRef { coref } => cx
            .entity
            .coref
            .as_ref()
            .is_some_and(|c| c.as_str() == coref.as_str()),
        Predicate::All { all } => all.iter().all(|p| eval(p, cx, context)),
        Predicate::Any { any } => any.iter().any(|p| eval(p, cx, context)),
        Predicate::Not { not } => !eval(not, cx, context),
    }
}
