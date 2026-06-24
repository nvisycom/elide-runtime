//! Compile text-modality rules to elide operators + attach to an
//! [`Anonymizer<Text>`].

use elide::Anonymizer;
use elide::redaction::operators::{Erase, Hash, HashAlgorithm, Keep, Mask, Replace};
use elide_core::entity::LabelCatalog;
use elide_core::modality::text::Text;
use elide_core::{Error, ErrorKind};
use nvisy_core::policy::redaction::{HashAlgorithm as PolicyHashAlgorithm, TextRedaction};
use nvisy_core::policy::{Policy, Rule, RuleAction};

use uuid::Uuid;

use super::selector::{attach, attach_override, fallback_attribution, rule_attribution};

/// Compile every text-applicable rule across `policies` into a
/// text-modality anonymizer. Walks policies in precedence order
/// and rules in declared order; the first matching rule wins per
/// entity at apply time.
pub fn compile_text(policies: &[Policy], catalog: LabelCatalog) -> Result<Anonymizer<Text>, Error> {
    let anonymizer = Anonymizer::<Text>::new().with_catalog(catalog);
    attach_policies_text(anonymizer, policies.iter())
}

/// Attach every text-applicable rule from `policies` onto an
/// already-constructed anonymizer. The apply pipeline calls this
/// after layering per-entity override rules so the overrides
/// keep first-match precedence. Takes an iterator (not a slice)
/// so callers can pre-filter by [`Policy::applies_when`] without
/// cloning the policy set.
///
/// [`Policy::applies_when`]: nvisy_core::policy::Policy::applies_when
pub(crate) fn attach_policies_text<'a>(
    mut anonymizer: Anonymizer<Text>,
    policies: impl Iterator<Item = &'a Policy>,
) -> Result<Anonymizer<Text>, Error> {
    for policy in policies {
        for rule in &policy.rules {
            let RuleAction::Redact(redactions) = &rule.action else {
                continue;
            };
            let Some(spec) = &redactions.text else {
                continue;
            };
            anonymizer = attach_rule(anonymizer, policy, rule, spec)?;
        }
        if let Some(RuleAction::Redact(redactions)) = &policy.fallback
            && let Some(spec) = &redactions.text
        {
            anonymizer = attach_fallback(anonymizer, policy, spec)?;
        }
    }
    Ok(anonymizer)
}

/// Attach a reviewer override for one entity. The override only
/// applies to `RuleAction::Redact` with a text-modality spec —
/// other action shapes (or non-text overrides) are no-ops at the
/// per-modality dispatch boundary.
pub(crate) fn attach_override_text(
    anonymizer: Anonymizer<Text>,
    entity_id: Uuid,
    action: &RuleAction,
) -> Result<Anonymizer<Text>, Error> {
    let RuleAction::Redact(redactions) = action else {
        return Ok(anonymizer);
    };
    let Some(spec) = &redactions.text else {
        return Ok(anonymizer);
    };
    Ok(match build(spec)? {
        TextOp::Erase => attach_override(anonymizer, entity_id, Erase),
        TextOp::Keep => attach_override(anonymizer, entity_id, Keep),
        TextOp::Mask(op) => attach_override(anonymizer, entity_id, op),
        TextOp::Replace(op) => attach_override(anonymizer, entity_id, op),
        TextOp::Hash(op) => attach_override(anonymizer, entity_id, op),
    })
}

fn attach_rule(
    anonymizer: Anonymizer<Text>,
    policy: &Policy,
    rule: &Rule,
    spec: &TextRedaction,
) -> Result<Anonymizer<Text>, Error> {
    let attribution = rule_attribution(policy, rule);
    Ok(match build(spec)? {
        TextOp::Erase => attach(anonymizer, &rule.predicate, Erase, attribution),
        TextOp::Keep => attach(anonymizer, &rule.predicate, Keep, attribution),
        TextOp::Mask(op) => attach(anonymizer, &rule.predicate, op, attribution),
        TextOp::Replace(op) => attach(anonymizer, &rule.predicate, op, attribution),
        TextOp::Hash(op) => attach(anonymizer, &rule.predicate, op, attribution),
    })
}

fn attach_fallback(
    anonymizer: Anonymizer<Text>,
    policy: &Policy,
    spec: &TextRedaction,
) -> Result<Anonymizer<Text>, Error> {
    let attribution = fallback_attribution(policy);
    Ok(match build(spec)? {
        TextOp::Erase => anonymizer.with_fallback(Erase).because(attribution),
        TextOp::Keep => anonymizer.with_fallback(Keep).because(attribution),
        TextOp::Mask(op) => anonymizer.with_fallback(op).because(attribution),
        TextOp::Replace(op) => anonymizer.with_fallback(op).because(attribution),
        TextOp::Hash(op) => anonymizer.with_fallback(op).because(attribution),
    })
}

/// Discriminated builder result so [`attach`] /
/// [`Anonymizer::with_fallback`] can attach the right concrete
/// operator type. We can't return `Box<dyn Operator<_>>` because
/// [`Anonymizer::with_label`] takes `O: Operator<M> + 'static` by
/// value.
enum TextOp {
    Erase,
    Keep,
    Mask(Mask),
    Replace(Replace),
    Hash(Hash),
}

fn build(spec: &TextRedaction) -> Result<TextOp, Error> {
    Ok(match spec {
        TextRedaction::Erase => TextOp::Erase,
        TextRedaction::Keep => TextOp::Keep,
        TextRedaction::Mask {
            mask_char,
            keep_prefix,
            keep_suffix,
        } => TextOp::Mask(
            Mask::new(*mask_char)
                .with_keep_prefix(*keep_prefix)
                .with_keep_suffix(*keep_suffix),
        ),
        TextRedaction::Replace { template } => TextOp::Replace(Replace::new(template.clone())),
        TextRedaction::Hash { algorithm, salt } => {
            let mut op = Hash::new(map_hash_algorithm(*algorithm));
            if let Some(s) = salt {
                op = op.with_salt(s.as_bytes().to_vec());
            }
            TextOp::Hash(op)
        }
        TextRedaction::Pseudonymize => {
            return Err(stateful_not_wired("pseudonymize", "vault + generator"));
        }
        TextRedaction::Encrypt => {
            return Err(stateful_not_wired("encrypt", "key provider"));
        }
    })
}

fn map_hash_algorithm(spec: PolicyHashAlgorithm) -> HashAlgorithm {
    match spec {
        PolicyHashAlgorithm::Sha256 => HashAlgorithm::Sha256,
        PolicyHashAlgorithm::Sha512 => HashAlgorithm::Sha512,
    }
}

fn stateful_not_wired(operator: &'static str, infrastructure: &'static str) -> Error {
    Error::new(
        ErrorKind::Validation,
        format!(
            "policy compile: `{operator}` needs an engine-side {infrastructure}; \
             stateful operator infrastructure is not wired into the compile surface yet",
        ),
    )
}
