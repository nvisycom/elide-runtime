//! Per-modality policy attachment, generic over the rule shape.
//!
//! Every per-modality `compile` follows the same outer shape:
//!
//! - Walk `policies` in precedence order.
//! - For each policy, walk `rules` in declared order. A rule
//!   whose redaction spec carries this modality compiles its
//!   operator and attaches it (rule predicate + attribution).
//! - Then a policy `fallback` with this modality's arm set
//!   attaches as a [`Rule::fallback`] on the anonymizer.
//!
//! The per-modality differences are exactly two: which
//! `redactions.{modality}` field to read, and how to build (then
//! attach) the typed operator. This module captures the
//! invariant outer loop and parameterises both via a callback so
//! every per-modality file stays under ~50 lines — just the
//! `Op` enum + `build` + an `attach_to` dispatcher.
//!
//! [`Rule::fallback`]: elide::redaction::Rule::fallback

use elide::redaction::{Anonymizer, Rule};
use elide_core::Result;
use elide_core::entity::provenance::Attribution;
use elide_core::modality::Modality;
use elide_core::operator::Operator;
use nvisy_schema::policy::PolicyDefinition;
use nvisy_schema::policy::predicate::Predicate;
use nvisy_schema::policy::redaction::ModalityRedactions;
use uuid::Uuid;

use super::selector::{attach, attach_override, fallback_attribution, rule_attribution};

/// Where an operator is being attached. The per-modality `Op`
/// dispatcher takes one of these and feeds the carried
/// `Anonymizer<M>` through the right elide entry point.
pub(in crate::anonymizer) enum Target<'a, M: Modality> {
    /// Rule attachment: predicate-guarded redaction with a
    /// name + description attribution. Maps to
    /// [`super::selector::attach`]. `policy_id` scopes any
    /// [`Predicate::LabelInGroup`] to the enclosing policy's
    /// synthetic tag namespace.
    Rule {
        anonymizer: Anonymizer<M>,
        predicate: &'a Predicate,
        attribution: Attribution,
        policy_id: Uuid,
    },
    /// PolicyDefinition `fallback`: catch-all redaction attached
    /// via [`Rule::fallback`] with a `because(fallback_attribution)`.
    ///
    /// [`Rule::fallback`]: elide::redaction::Rule::fallback
    Fallback {
        anonymizer: Anonymizer<M>,
        attribution: Attribution,
    },
    /// Reviewer override: per-entity, sentinel attribution. Maps
    /// to [`super::selector::attach_override`].
    Override {
        anonymizer: Anonymizer<M>,
        entity_id: Uuid,
    },
}

impl<'a, M: Modality + 'static> Target<'a, M> {
    /// Attach `operator` to whichever target this is. Centralises
    /// the per-target dispatch so each per-modality `Op` enum
    /// needs exactly one match on its own variants — no separate
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
                policy_id,
            } => attach(anonymizer, predicate, operator, attribution, policy_id),
            Target::Fallback {
                anonymizer,
                attribution,
            } => anonymizer.with(Rule::fallback(operator).because(attribution)),
            Target::Override {
                anonymizer,
                entity_id,
            } => attach_override(anonymizer, entity_id, operator),
        }
    }

    /// Recover the anonymizer unchanged. Per-modality callbacks
    /// use this when this modality's redaction slot is absent —
    /// they take no action, return the input.
    pub(in crate::anonymizer) fn passthrough(self) -> Anonymizer<M> {
        match self {
            Target::Rule { anonymizer, .. } => anonymizer,
            Target::Fallback { anonymizer, .. } => anonymizer,
            Target::Override { anonymizer, .. } => anonymizer,
        }
    }
}

/// Walk `policies` and feed each rule + each fallback through
/// `compile_one`. `compile_one` is the per-modality bridge: read
/// `Redactions::{modality}`, build the operator, and dispatch
/// onto the `Target`.
///
/// `compile_one` returns `Ok(anonymizer)` even when the
/// redaction spec is absent or wrong-modality — those are no-ops
/// at the per-modality dispatch boundary. Operator build errors
/// propagate.
pub(in crate::anonymizer) fn attach_policies<'a, M, F>(
    mut anonymizer: Anonymizer<M>,
    policies: impl Iterator<Item = &'a PolicyDefinition>,
    mut compile_one: F,
) -> Result<Anonymizer<M>>
where
    M: Modality + 'static,
    F: FnMut(Target<'_, M>, &ModalityRedactions) -> Result<Anonymizer<M>>,
{
    for policy in policies {
        for rule in &policy.rules {
            let attribution = rule_attribution(policy, rule);
            for (predicate, action) in rule.attachments() {
                anonymizer = compile_one(
                    Target::Rule {
                        anonymizer,
                        predicate: &predicate,
                        attribution: attribution.clone(),
                        policy_id: policy.id,
                    },
                    action,
                )?;
            }
        }
        if let Some(redactions) = &policy.fallback {
            anonymizer = compile_one(
                Target::Fallback {
                    anonymizer,
                    attribution: fallback_attribution(policy),
                },
                redactions,
            )?;
        }
    }
    Ok(anonymizer)
}

/// Apply a reviewer override on one entity. `compile_one` is the
/// same per-modality bridge as in [`attach_policies`]; called
/// once with a [`Target::Override`].
pub(in crate::anonymizer) fn attach_one_override<M, F>(
    anonymizer: Anonymizer<M>,
    entity_id: Uuid,
    redactions: &ModalityRedactions,
    compile_one: F,
) -> Result<Anonymizer<M>>
where
    M: Modality + 'static,
    F: FnOnce(Target<'_, M>, &ModalityRedactions) -> Result<Anonymizer<M>>,
{
    compile_one(
        Target::Override {
            anonymizer,
            entity_id,
        },
        redactions,
    )
}
