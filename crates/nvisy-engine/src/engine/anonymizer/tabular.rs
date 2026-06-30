//! Compile tabular-modality rules to elide operators + attach to
//! an [`Anonymizer<Tabular>`].
//!
//! Tabular shares the [`TextBacked`] vocabulary with text plus the
//! structural [`DropRow`] / [`DropColumn`] operators.
//!
//! [`TextBacked`]: elide_core::modality::TextBacked
//! [`DropRow`]: elide::redaction::operators::DropRow
//! [`DropColumn`]: elide::redaction::operators::DropColumn

use elide::redaction::Anonymizer;
use elide::redaction::operators::{DropColumn, DropRow, Erase, Keep, Mask, Replace, Sha2Hash};
use elide_core::modality::tabular::Tabular;
use elide_core::{Error, ErrorKind};
use nvisy_core::policy::RuleAction;
use nvisy_core::policy::redaction::{ModalityRedactions, TabularRedaction};
use uuid::Uuid;

use super::dispatch::{Target, attach_one_override, attach_policies};

/// Attach every tabular-applicable rule from `policies` onto an
/// already-constructed anonymizer. Takes an iterator so the
/// apply pipeline can pre-filter by [`Policy::applies_when`]
/// without cloning.
///
/// [`Policy::applies_when`]: nvisy_core::policy::Policy::applies_when
pub(crate) fn attach_policies_tabular<'a>(
    anonymizer: Anonymizer<Tabular>,
    policies: impl Iterator<Item = &'a nvisy_core::policy::Policy>,
) -> Result<Anonymizer<Tabular>, Error> {
    attach_policies(anonymizer, policies, compile_one)
}

/// Attach a reviewer override for one entity. No-op when the
/// override is not a tabular-modality `Redact`.
pub(crate) fn attach_override_tabular(
    anonymizer: Anonymizer<Tabular>,
    entity_id: Uuid,
    action: &RuleAction,
) -> Result<Anonymizer<Tabular>, Error> {
    attach_one_override(anonymizer, entity_id, action, compile_one)
}

fn compile_one(
    target: Target<'_, Tabular>,
    redactions: &ModalityRedactions,
) -> Result<Anonymizer<Tabular>, Error> {
    let Some(spec) = &redactions.tabular else {
        return Ok(target.passthrough());
    };
    Ok(match build(spec)? {
        TabularOp::Erase => target.attach_with(Erase),
        TabularOp::Keep => target.attach_with(Keep),
        TabularOp::Mask(op) => target.attach_with(op),
        TabularOp::Replace(op) => target.attach_with(op),
        TabularOp::Hash(op) => target.attach_with(op),
        TabularOp::DropRow => target.attach_with(DropRow),
        TabularOp::DropColumn => target.attach_with(DropColumn),
    })
}

enum TabularOp {
    Erase,
    Keep,
    Mask(Mask),
    Replace(Replace),
    Hash(Sha2Hash),
    DropRow,
    DropColumn,
}

fn build(spec: &TabularRedaction) -> Result<TabularOp, Error> {
    Ok(match spec {
        TabularRedaction::Erase => TabularOp::Erase,
        TabularRedaction::Keep => TabularOp::Keep,
        TabularRedaction::Mask {
            mask_char,
            keep_prefix,
            keep_suffix,
        } => TabularOp::Mask(
            Mask::new(*mask_char)
                .with_keep_prefix(*keep_prefix)
                .with_keep_suffix(*keep_suffix),
        ),
        TabularRedaction::Replace { template } => {
            TabularOp::Replace(Replace::new(template.clone()))
        }
        TabularRedaction::Hash { algorithm, salt } => {
            let mut op = Sha2Hash::new((*algorithm).into());
            if let Some(s) = salt {
                op = op.with_salt(s.as_bytes().to_vec());
            }
            TabularOp::Hash(op)
        }
        TabularRedaction::DropRow => TabularOp::DropRow,
        TabularRedaction::DropColumn => TabularOp::DropColumn,
        TabularRedaction::Pseudonymize => return Err(stateful_not_wired("pseudonymize")),
        TabularRedaction::Encrypt => return Err(stateful_not_wired("encrypt")),
    })
}

fn stateful_not_wired(operator: &'static str) -> Error {
    Error::new(
        ErrorKind::Validation,
        format!(
            "policy compile: `{operator}` needs engine-side stateful infrastructure (vault / \
             key provider) that is not wired into the compile surface yet",
        ),
    )
}
