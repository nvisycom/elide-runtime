//! Compile [`TextRedaction`] specs to elide text operators.

use elide::Anonymizer;
use elide::redaction::operators::{Erase, Hash, HashAlgorithm, Keep, Mask, Replace};
use elide_core::entity::LabelCatalog;
use elide_core::modality::text::Text;
use elide_core::redaction::Attribution;
use elide_core::{Error, ErrorKind};
use nvisy_core::policy::redaction::{HashAlgorithm as PolicyHashAlgorithm, TextRedaction};
use nvisy_core::policy::{Action, EntitySelector, Policy};

use super::selector::{attach, default_attribution, rule_attribution};

/// Compile every text-rule across `policies` into a text-modality
/// anonymizer. Walks policies in precedence order and rules in
/// declared order; every attached rule stamps an
/// [`Attribution`] that flows onto each redaction's provenance.
pub fn compile_text(policies: &[Policy], catalog: LabelCatalog) -> Result<Anonymizer<Text>, Error> {
    let mut anonymizer = Anonymizer::<Text>::new().with_catalog(catalog);
    for policy in policies {
        for rule in &policy.rules {
            if !rule.enabled {
                continue;
            }
            let Action::Redact(redactions) = &rule.action else {
                continue;
            };
            let Some(spec) = &redactions.text else {
                continue;
            };
            anonymizer =
                attach_text(anonymizer, &rule.selector, spec, rule_attribution(policy, rule))?;
        }
        if let Some(Action::Redact(redactions)) = &policy.default_action
            && let Some(spec) = &redactions.text
        {
            anonymizer = attach_text_fallback(anonymizer, spec, default_attribution(policy))?;
        }
    }
    Ok(anonymizer)
}

fn attach_text(
    anonymizer: Anonymizer<Text>,
    selector: &EntitySelector,
    spec: &TextRedaction,
    attribution: Attribution,
) -> Result<Anonymizer<Text>, Error> {
    Ok(match build(spec)? {
        TextOp::Erase => attach(anonymizer, selector, Erase, attribution),
        TextOp::Keep => attach(anonymizer, selector, Keep, attribution),
        TextOp::Mask(op) => attach(anonymizer, selector, op, attribution),
        TextOp::Replace(op) => attach(anonymizer, selector, op, attribution),
        TextOp::Hash(op) => attach(anonymizer, selector, op, attribution),
    })
}

fn attach_text_fallback(
    anonymizer: Anonymizer<Text>,
    spec: &TextRedaction,
    attribution: Attribution,
) -> Result<Anonymizer<Text>, Error> {
    Ok(match build(spec)? {
        TextOp::Erase => anonymizer.with_fallback(Erase).because(attribution),
        TextOp::Keep => anonymizer.with_fallback(Keep).because(attribution),
        TextOp::Mask(op) => anonymizer.with_fallback(op).because(attribution),
        TextOp::Replace(op) => anonymizer.with_fallback(op).because(attribution),
        TextOp::Hash(op) => anonymizer.with_fallback(op).because(attribution),
    })
}

/// Discriminated builder result so [`attach`] / [`Anonymizer::with_fallback`]
/// can attach the right concrete operator type. We can't return
/// `Box<dyn Operator<_>>` because [`Anonymizer::with_label`] takes
/// `O: Operator<M> + 'static` by value.
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
