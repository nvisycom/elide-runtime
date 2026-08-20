//! Per-modality policy attachment, generic over the rule shape.
//!
//! Every per-modality `compile` follows the same outer shape:
//!
//! - **Rules pass.** Walk every policy in submission order and
//!   attach every rule (predicate + attribution + per-policy
//!   scope) in the order it was declared. Elide's anonymizer
//!   evaluates rules top-to-bottom with first-match-wins, so a
//!   policy A rule at slot N wins over a policy B rule at slot
//!   N+1 for entities in both policies' scope.
//! - **Fallbacks pass.** Walk every policy again and attach every
//!   `fallback` after all rules. Attaching fallbacks second: not
//!   interleaved between policies: is load-bearing: a policy A
//!   fallback attached before policy B's rules would fire on any
//!   in-A-scope entity before B's more specific rules had a
//!   chance, silently shadowing B.
//!
//! The per-modality differences are exactly two: which
//! `redactions.{modality}` field to read, and how to build (then
//! attach) the typed operator. This module captures the
//! invariant outer loop and parameterises both via a callback so
//! every per-modality file stays under ~50 lines: just the
//! `Op` enum + `build` + an `attach_to` dispatcher.
//!
//! [`Rule::fallback`]: elide::redaction::Rule::fallback

use elide::Result;
use elide::entity::audit::Attribution;
use elide::modality::Modality;
use elide::redaction::{Anonymizer, MatchContext, Operator, Rule};
use elide_governance::redaction::ModalityRedactions;
use elide_governance::{PolicyDefinition, Predicate};
use uuid::Uuid;

use super::selector::{
    PolicyContext, attach, attach_override, fallback_attribution, rule_attribution,
};
use crate::entity::OverrideEntry;

/// Where an operator is being attached. The per-modality `Op`
/// dispatcher takes one of these and feeds the carried
/// `Anonymizer<M>` through the right elide entry point.
pub(in crate::anonymizer) enum Target<'a, M: Modality> {
    /// Rule attachment: predicate-guarded redaction with a
    /// name + description attribution. Maps to
    /// [`super::selector::attach`]. `context` carries the
    /// enclosing policy's per-policy label scope and group table.
    Rule {
        anonymizer: Anonymizer<M>,
        predicate: &'a Predicate,
        attribution: Attribution,
        context: PolicyContext,
    },
    /// PolicyDefinition `fallback`: catch-all redaction attached
    /// via [`Rule::fallback`], but filtered by the enclosing
    /// policy's label scope so a fallback fires only on entities
    /// the policy actually declared vocabulary for.
    ///
    /// [`Rule::fallback`]: elide::redaction::Rule::fallback
    Fallback {
        anonymizer: Anonymizer<M>,
        attribution: Attribution,
        context: PolicyContext,
    },
    /// Reviewer override: per-entity, carries the overriding
    /// policy's authority. Maps to
    /// [`super::selector::attach_override`]. The `policy_id`
    /// scopes any per-policy operator infrastructure the override
    /// pulls in (per-policy pseudonym vault, per-policy
    /// `KeyProvider`) and gets stamped on the audit attribution
    /// so reviewers can trace back which policy the override
    /// exercised authority under.
    Override {
        anonymizer: Anonymizer<M>,
        entity_id: Uuid,
        policy_id: Uuid,
    },
}

impl<'a, M: Modality + 'static> Target<'a, M> {
    /// Attach `operator` to whichever target this is. Centralises
    /// the per-target dispatch so each per-modality `Op` enum
    /// needs exactly one match on its own variants: no separate
    /// `attach_rule` / `attach_fallback` / `attach_override`
    /// dispatchers per modality.
    pub(in crate::anonymizer) fn attach_with<O>(self, operator: O) -> Anonymizer<M>
    where
        O: Operator<M> + 'static,
    {
        match self {
            Target::Rule {
                anonymizer,
                predicate,
                attribution,
                context,
            } => attach(anonymizer, predicate, operator, attribution, context),
            Target::Fallback {
                anonymizer,
                attribution,
                context,
            } => attach_fallback(anonymizer, operator, attribution, context),
            Target::Override {
                anonymizer,
                entity_id,
                policy_id,
            } => attach_override(anonymizer, entity_id, operator, policy_id),
        }
    }

    /// Recover the anonymizer unchanged. Per-modality callbacks
    /// use this when this modality's redaction slot is absent -
    /// they take no action, return the input.
    pub(in crate::anonymizer) fn passthrough(self) -> Anonymizer<M> {
        match self {
            Target::Rule { anonymizer, .. } => anonymizer,
            Target::Fallback { anonymizer, .. } => anonymizer,
            Target::Override { anonymizer, .. } => anonymizer,
        }
    }

    /// The policy authority under which this target attaches its
    /// operator. Per-policy operator infrastructure (pseudonym
    /// vault, `KeyProvider`) is keyed by this UUID.
    pub(in crate::anonymizer) fn policy_id(&self) -> Uuid {
        match self {
            Target::Rule { context, .. } => context.policy_id,
            Target::Fallback { context, .. } => context.policy_id,
            Target::Override { policy_id, .. } => *policy_id,
        }
    }
}

/// Attach the policy's `fallback` operator, scoped so it only
/// fires on entities whose label the policy actually declared.
///
/// Compiles to a plain [`Rule::predicate`] rather than elide's
/// [`Rule::fallback`] for two reasons:
///
/// - Elide's `Rule::fallback` uses `Matcher::Always`, making
///   every subsequent rule (including later policies')
///   unreachable, so a per-policy fallback wired that way would
///   shadow every policy declared after it.
/// - A `Rule::fallback` runs on every unmatched entity in the
///   request, ignoring the enclosing policy's declared vocabulary,
///   which would silently redact entities the policy never named.
///
/// The scoped-predicate form fires on the enclosing policy's own
/// unmatched entities only. First-match-wins still holds within
/// the policy (rules attach before the fallback), and later
/// policies remain reachable.
///
/// [`Rule::predicate`]: elide::redaction::Rule::predicate
/// [`Rule::fallback`]: elide::redaction::Rule::fallback
fn attach_fallback<M, O>(
    anonymizer: Anonymizer<M>,
    operator: O,
    attribution: Attribution,
    context: PolicyContext,
) -> Anonymizer<M>
where
    M: Modality + 'static,
    O: Operator<M> + 'static,
{
    let scoped = move |cx: &MatchContext<'_, M>| context.label_scope_contains(&cx.entity.label);
    anonymizer.with(Rule::predicate(scoped, operator).because(attribution))
}

/// Walk `policies` and feed each rule + each fallback through
/// `compile_one` in two passes: rules first (every policy),
/// then fallbacks (every policy). `compile_one` is the per-
/// modality bridge: read `Redactions::{modality}`, build the
/// operator, and dispatch onto the `Target`.
///
/// The two-pass structure is load-bearing for cross-policy
/// composition. Attaching each policy's fallback immediately
/// after its own rules: before the next policy's rules: would
/// let a coarse baseline policy's fallback silently shadow every
/// later policy's more specific rules on any label the baseline
/// declared. Fallbacks pass last so they fire only after every
/// policy has had a shot at the entity.
///
/// `compile_one` returns `Ok(anonymizer)` even when the
/// redaction spec is absent or wrong-modality: those are no-ops
/// at the per-modality dispatch boundary. Operator build errors
/// propagate.
pub(in crate::anonymizer) fn attach_policies<'a, M, F>(
    mut anonymizer: Anonymizer<M>,
    policies: impl Iterator<Item = &'a PolicyDefinition> + Clone,
    mut compile_one: F,
) -> Result<Anonymizer<M>>
where
    M: Modality + 'static,
    F: FnMut(Target<'_, M>, &ModalityRedactions) -> Result<Anonymizer<M>>,
{
    // Rules pass.
    for policy in policies.clone() {
        let context = PolicyContext::from_policy(policy);
        for rule in &policy.rules {
            let attribution = rule_attribution(policy, rule);
            for (predicate, action) in rule.attachments() {
                anonymizer = compile_one(
                    Target::Rule {
                        anonymizer,
                        predicate: &predicate,
                        attribution: attribution.clone(),
                        context: context.clone(),
                    },
                    action,
                )?;
            }
        }
    }
    // Fallbacks pass.
    for policy in policies {
        if let Some(redactions) = &policy.fallback {
            let context = PolicyContext::from_policy(policy);
            anonymizer = compile_one(
                Target::Fallback {
                    anonymizer,
                    attribution: fallback_attribution(policy),
                    context,
                },
                redactions,
            )?;
        }
    }
    Ok(anonymizer)
}

/// Apply a reviewer override on one entity, using the policy id
/// carried on the [`OverrideEntry`] as the override's authority.
/// `compile_one` is the same per-modality bridge as in
/// [`attach_policies`]; called once with a [`Target::Override`].
///
/// [`OverrideEntry`]: crate::entity::OverrideEntry
pub(in crate::anonymizer) fn attach_one_override<M, F>(
    anonymizer: Anonymizer<M>,
    entry: &OverrideEntry,
    compile_one: F,
) -> Result<Anonymizer<M>>
where
    M: Modality + 'static,
    F: FnOnce(Target<'_, M>, &ModalityRedactions) -> Result<Anonymizer<M>>,
{
    compile_one(
        Target::Override {
            anonymizer,
            entity_id: entry.entity_id,
            policy_id: entry.policy_id,
        },
        &entry.action,
    )
}
