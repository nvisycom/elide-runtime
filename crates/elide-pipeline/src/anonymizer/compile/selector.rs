//! Attach a [`PolicyRule`] / `fallback` onto an elide [`Anonymizer`],
//! stamping the engine-internal attribution from policy + rule
//! UUIDs and enforcing per-policy label + group scoping.
//!
//! Every predicate compiles into a closure passed to
//! [`Rule::predicate`]. A rule declared inside policy A can only
//! fire on entities whose label is in `policy.labels` (its own
//! declared vocabulary), and any [`Predicate::LabelInScope`] leaf
//! resolves against `policy.scopes`, not the shared label catalog.
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
//! [`PolicyRule`]: elide_governance::PolicyRule
//! [`Predicate`]: elide_governance::Predicate
//! [`Predicate::LabelInScope`]: elide_governance::Predicate::LabelInScope
//! [`Predicate::TagOneOf`]: elide_governance::Predicate::TagOneOf
//! [`Rule::label`]: elide::redaction::Rule::label
//! [`Rule::predicate`]: elide::redaction::Rule::predicate
//! [`Rule::tag`]: elide::redaction::Rule::tag
//!
//! Attribution always sits in elide's [`Attribution`] slot:
//! `name` from the policy's UUID, `description` from the rule's
//! UUID (or omitted for the policy fallback).
//!
//! [`Attribution`]: elide_core::entity::audit::Attribution

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use elide::redaction::{Anonymizer, MatchContext, Rule};
use elide_core::entity::LabelRef;
use elide_core::entity::audit::Attribution;
use elide_core::modality::Modality;
use elide_core::operator::Operator;
use elide_governance::{LabelScope, PolicyDefinition, PolicyRule, Predicate};
use uuid::Uuid;

/// Per-policy scoping context threaded into every predicate the
/// selector compiles. Built once per policy at attach time; every
/// rule inside that policy shares the same reference-counted copy.
///
/// - `label_scope`: the [`LabelRef`]s the policy declares in its
///   [`labels`] block. Every predicate filters by whether the
///   candidate entity's label is in this set, so a policy that
///   lists only `email_address` never fires on a `phone_number`
///   another policy pulled into the request's recognition pass.
/// - `scopes`: resolved scope name → labelset lookup, materialised
///   from the policy's [`scopes`] once. A rule can only
///   name a scope its own policy declared (validated separately
///   in `pipeline::orchestrator::validate_scope_references`).
///
/// [`label_scope`]: elide_governance::PolicyDefinition::label_scope
/// [`scopes`]: elide_governance::PolicyDefinition::scopes
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
    scopes: Arc<HashMap<String, HashSet<LabelRef>>>,
}

impl PolicyContext {
    /// Materialise a policy's scoping context from its declared
    /// [`scopes`] and inline custom schemas.
    ///
    /// [`scopes`]: elide_governance::PolicyDefinition::scopes
    pub(in crate::anonymizer) fn from_policy(policy: &PolicyDefinition) -> Self {
        let label_scope: HashSet<LabelRef> = policy.label_scope().into_iter().collect();
        let scopes: HashMap<String, HashSet<LabelRef>> =
            policy.scopes.iter().map(scope_lookup_entry).collect();
        Self {
            policy_id: policy.id,
            label_scope: Arc::new(label_scope),
            scopes: Arc::new(scopes),
        }
    }

    /// Whether the enclosing policy declared vocabulary for
    /// `label`. The gate every predicate: including the policy
    /// fallback: passes through before any per-predicate logic
    /// evaluates.
    pub(in crate::anonymizer) fn label_scope_contains(&self, label: &LabelRef) -> bool {
        self.label_scope.contains(label)
    }
}

fn scope_lookup_entry(scope: &LabelScope) -> (String, HashSet<LabelRef>) {
    let labels: HashSet<LabelRef> = scope.labels.iter().cloned().collect();
    (scope.name.to_string(), labels)
}

/// Build an [`Attribution`] for a concrete rule that fired.
///
/// A rule carrying a attribution passes it through verbatim, so
/// an audit records the provision rather than prose a consumer
/// would have to parse.
///
/// Without one the attribution falls back to [`Freeform`] under
/// the policy's own name and description.
///
/// Either way `source_id` carries the rule's UUID: it is
/// orthogonal to whether the author supplied a citation, and it
/// is what keeps two rules citing the same provision (Safe
/// Harbor's age/date table and its bulk erase both cite
/// §164.514(b)(2)) distinguishable in the trail.
///
/// [`Freeform`]: elide_core::entity::audit::AttributionKind::Freeform
pub(super) fn rule_attribution(policy: &PolicyDefinition, rule: &PolicyRule) -> Attribution {
    let attribution = match &rule.attribution {
        // No public constructor takes a prebuilt kind, and both
        // fields are public, so a literal beats destructuring the
        // enum only to rebuild it.
        Some(kind) => Attribution {
            kind: kind.clone(),
            source_id: None,
        },
        None => {
            let freeform = Attribution::freeform(policy.name.clone());
            match &policy.description {
                Some(description) => freeform.with_description(description.clone()),
                None => freeform,
            }
        }
    };
    attribution.with_source_id(rule.id)
}

/// Build an [`Attribution`] for a policy's `fallback`.
///
/// Freeform under the policy's own name: a catch-all fires
/// because no rule claimed the entity, so there is no provision
/// to cite. No `source_id` either, since no rule fired.
pub(super) fn fallback_attribution(policy: &PolicyDefinition) -> Attribution {
    // A policy whose scopes all answer to one authority can cite it:
    // CCPA and GDPR do every redaction through the fallback, so
    // without this their audit events would lose the citation their
    // scope carries.
    //
    // Compares `Option`s rather than filtering the uncited ones out,
    // so a cited scope beside an uncited one disagrees. Skipping the
    // uncited scope would stamp its labels with an authority that
    // does not cover them, which is worse than no citation at all.
    let mut declared = policy.scopes.iter().map(|s| s.attribution.as_ref());
    if let Some(Some(first)) = declared.next()
        && declared.all(|other| other == Some(first))
    {
        return Attribution {
            kind: first.clone(),
            source_id: None,
        };
    }
    Attribution::freeform(policy.name.clone())
        .with_description("policy fallback: no rule matched this entity")
}

/// Build an [`Attribution`] for a reviewer override.
///
/// A reviewer's decision cites no provision, so it is freeform,
/// named for the policy whose authority the reviewer exercised.
/// `source_id` carries the overridden entity rather than a rule
/// id, since the entity *is* the source record here, and the
/// description marks the event as reviewer-driven so audits
/// separate the two at a glance.
pub(super) fn override_attribution(entity_id: Uuid, policy_id: Uuid) -> Attribution {
    Attribution::freeform(policy_id.to_string())
        .with_description("reviewer override")
        .with_source_id(entity_id)
}

/// Attach an `operator` to `anonymizer` for the single entity
/// identified by `entity_id`. Used by the apply pipeline to give
/// reviewer overrides higher precedence than any policy-driven
/// rule. `policy_id` names the overriding policy: attribution
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
/// [`Predicate`]: elide_governance::Predicate
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
/// evaluating the tree; any [`Predicate::LabelInScope`] leaf
/// resolves against the policy's own materialised scope table.
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
        Predicate::LabelInScope { scope } => context
            .scopes
            .get(scope.as_str())
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
