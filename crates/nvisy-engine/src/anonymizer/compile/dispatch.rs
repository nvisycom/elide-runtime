//! Per-modality policy attachment, generic over the rule shape.
//!
//! Every per-modality `compile` follows the same outer shape:
//!
//! - Walk `policies` in precedence order.
//! - For each policy, walk `rules` in declared order. A rule with
//!   a `Redact` action whose redaction spec carries this modality
//!   compiles its operator and attaches it (rule predicate +
//!   attribution).
//! - Then a policy `fallback` of `Redact` with this modality's
//!   arm set becomes the anonymizer's [`with_fallback`].
//!
//! The per-modality differences are exactly two: which
//! `redactions.{modality}` field to read, and how to build (then
//! attach) the typed operator. This module captures the
//! invariant outer loop and parameterises both via a callback so
//! every per-modality file stays under ~50 lines — just the
//! `Op` enum + `build` + an `attach_to` dispatcher.
//!
//! [`with_fallback`]: elide::redaction::Anonymizer::with_fallback

use elide::redaction::Anonymizer;
use elide_core::Error;
use elide_core::entity::provenance::Attribution;
use elide_core::modality::Modality;
use elide_core::operator::Operator;
use nvisy_schema::policy::predicate::Predicate;
use nvisy_schema::policy::redaction::ModalityRedactions;
use nvisy_schema::policy::{Policy, PolicyAction};
use uuid::Uuid;

use super::selector::{attach, attach_override, fallback_attribution, rule_attribution};

/// Where an operator is being attached. The per-modality `Op`
/// dispatcher takes one of these and feeds the carried
/// `Anonymizer<M>` through the right elide entry point.
pub(in crate::anonymizer) enum Target<'a, M: Modality> {
    /// Rule attachment: predicate-guarded redaction with a
    /// `policy_id` + `reason` attribution. Maps to
    /// [`super::selector::attach`].
    Rule {
        anonymizer: Anonymizer<M>,
        predicate: &'a Predicate,
        attribution: Attribution,
    },
    /// Policy `fallback`: catch-all redaction with `reason: None`.
    /// Maps to [`Anonymizer::with_fallback`] + `because`.
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
        O: Operator<M> + Clone + 'static,
    {
        match self {
            Target::Rule {
                anonymizer,
                predicate,
                attribution,
            } => attach(anonymizer, predicate, operator, attribution),
            Target::Fallback {
                anonymizer,
                attribution,
            } => anonymizer.with_fallback(operator).because(attribution),
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
    policies: impl Iterator<Item = &'a Policy>,
    mut compile_one: F,
) -> Result<Anonymizer<M>, Error>
where
    M: Modality + 'static,
    F: FnMut(Target<'_, M>, &ModalityRedactions) -> Result<Anonymizer<M>, Error>,
{
    for policy in policies {
        for rule in &policy.rules {
            let PolicyAction::Redact(redactions) = &rule.action else {
                continue;
            };
            anonymizer = compile_one(
                Target::Rule {
                    anonymizer,
                    predicate: &rule.predicate,
                    attribution: rule_attribution(policy, rule),
                },
                redactions,
            )?;
        }
        if let Some(PolicyAction::Redact(redactions)) = &policy.fallback {
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
    action: &PolicyAction,
    compile_one: F,
) -> Result<Anonymizer<M>, Error>
where
    M: Modality + 'static,
    F: FnOnce(Target<'_, M>, &ModalityRedactions) -> Result<Anonymizer<M>, Error>,
{
    let PolicyAction::Redact(redactions) = action else {
        return Ok(anonymizer);
    };
    compile_one(
        Target::Override {
            anonymizer,
            entity_id,
        },
        redactions,
    )
}
