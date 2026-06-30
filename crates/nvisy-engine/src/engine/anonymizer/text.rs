//! Compile text-modality rules to elide operators + attach to an
//! [`Anonymizer<Text>`].

use elide::redaction::Anonymizer;
use elide::redaction::operators::{Erase, Keep, Mask, Replace, Sha2Hash};
use elide_core::modality::text::Text;
use elide_core::{Error, ErrorKind};
use nvisy_core::policy::RuleAction;
use nvisy_core::policy::redaction::{ModalityRedactions, TextRedaction};
use uuid::Uuid;

use super::dispatch::{Target, attach_one_override, attach_policies};

/// Attach every text-applicable rule from `policies` onto an
/// already-constructed anonymizer. The apply pipeline calls this
/// after layering per-entity override rules so the overrides
/// keep first-match precedence. Takes an iterator (not a slice)
/// so callers can pre-filter by [`Policy::applies_when`] without
/// cloning the policy set.
///
/// [`Policy::applies_when`]: nvisy_core::policy::Policy::applies_when
pub(crate) fn attach_policies_text<'a>(
    anonymizer: Anonymizer<Text>,
    policies: impl Iterator<Item = &'a nvisy_core::policy::Policy>,
) -> Result<Anonymizer<Text>, Error> {
    attach_policies(anonymizer, policies, compile_one)
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
    attach_one_override(anonymizer, entity_id, action, compile_one)
}

fn compile_one(
    target: Target<'_, Text>,
    redactions: &ModalityRedactions,
) -> Result<Anonymizer<Text>, Error> {
    let Some(spec) = &redactions.text else {
        return Ok(target.passthrough());
    };
    Ok(match build(spec)? {
        TextOp::Erase => target.attach_with(Erase),
        TextOp::Keep => target.attach_with(Keep),
        TextOp::Mask(op) => target.attach_with(op),
        TextOp::Replace(op) => target.attach_with(op),
        TextOp::Hash(op) => target.attach_with(op),
    })
}

/// Discriminated builder result so [`Target::attach_with`] can
/// attach the right concrete operator type. We can't return
/// `Box<dyn Operator<_>>` because [`Anonymizer::with_label`]
/// takes `O: Operator<M> + 'static` by value.
enum TextOp {
    Erase,
    Keep,
    Mask(Mask),
    Replace(Replace),
    Hash(Sha2Hash),
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
            let mut op = Sha2Hash::new((*algorithm).into());
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

fn stateful_not_wired(operator: &'static str, infrastructure: &'static str) -> Error {
    Error::new(
        ErrorKind::Validation,
        format!(
            "policy compile: `{operator}` needs an engine-side {infrastructure}; \
             stateful operator infrastructure is not wired into the compile surface yet",
        ),
    )
}
