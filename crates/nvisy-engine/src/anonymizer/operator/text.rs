//! Text-modality operator builder, shared with `tabular`.
//!
//! Cells in a tabular are `TextBacked` in elide, so the same
//! concrete operators (`Erase`, `Keep`, `Mask`, `Replace`,
//! `Sha2Hash`) implement `Operator<Text>` *and*
//! `Operator<Tabular>`. This module owns the `TextRedaction ->
//! elide operator` bridge once; each per-modality file dispatches
//! its variant onto a [`Target`] with the built operator.

use elide::redaction::Anonymizer;
use elide::redaction::operators::{Erase, Keep, Mask, Replace, Sha2Algorithm, Sha2Hash};
use elide_core::modality::Modality;
use elide_core::operator::Operator;
use elide_core::{Error, ErrorKind};
use nvisy_schema::policy::redaction::{HashAlgorithm, TextRedaction};

use crate::anonymizer::compile::Target;

/// Runtime conversion from the wire's [`HashAlgorithm`] to
/// elide's [`Sha2Algorithm`]. Lives here rather than as a
/// `From` impl on the wire type so `nvisy-schema` stays free
/// of the `elide-redaction` dep.
fn to_sha2(algorithm: HashAlgorithm) -> Sha2Algorithm {
    match algorithm {
        HashAlgorithm::Sha256 => Sha2Algorithm::Sha256,
        HashAlgorithm::Sha512 => Sha2Algorithm::Sha512,
    }
}

/// Discriminated builder result so [`Target::attach_with`] can
/// attach the right concrete operator type. We can't return
/// `Box<dyn Operator<_>>` because [`Anonymizer::with_label`]
/// takes `O: Operator<M> + 'static` by value.
///
/// [`Anonymizer::with_label`]: elide::redaction::Anonymizer::with_label
pub(in crate::anonymizer) enum TextOp {
    Erase,
    Keep,
    Mask(Mask),
    Replace(Replace),
    Hash(Sha2Hash),
}

impl TextOp {
    /// Attach `self` to `target`. Works for any modality whose
    /// anonymizer accepts the text operator set (elide ships
    /// `impl Operator<Text>` and `impl Operator<Tabular>` on all
    /// five concrete ops).
    pub(in crate::anonymizer) fn attach_to<M>(self, target: Target<'_, M>) -> Anonymizer<M>
    where
        M: Modality + 'static,
        Erase: Operator<M>,
        Keep: Operator<M>,
        Mask: Operator<M> + Clone,
        Replace: Operator<M> + Clone,
        Sha2Hash: Operator<M> + Clone,
    {
        match self {
            TextOp::Erase => target.attach_with(Erase),
            TextOp::Keep => target.attach_with(Keep),
            TextOp::Mask(op) => target.attach_with(op),
            TextOp::Replace(op) => target.attach_with(op),
            TextOp::Hash(op) => target.attach_with(op),
        }
    }
}

/// Build a [`TextOp`] from the wire spec.
///
/// `Pseudonymize` and `Encrypt` need engine-side infrastructure
/// (vault, key provider) that isn't wired yet, so they error at
/// compile time — the wire declares them, the runtime rejects
/// them.
pub(in crate::anonymizer) fn build(spec: &TextRedaction) -> Result<TextOp, Error> {
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
            let mut op = Sha2Hash::new(to_sha2(*algorithm));
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
