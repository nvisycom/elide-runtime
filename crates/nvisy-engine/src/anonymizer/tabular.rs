//! Compile [`TabularRedaction`] specs to elide tabular operators.
//!
//! Tabular shares the [`TextBacked`] vocabulary with text plus the
//! structural [`DropRow`] / [`DropColumn`] operators.
//!
//! [`TextBacked`]: elide_core::modality::TextBacked
//! [`DropRow`]: elide::redaction::operators::DropRow
//! [`DropColumn`]: elide::redaction::operators::DropColumn

use elide::Anonymizer;
use elide::redaction::operators::{
    DropColumn, DropRow, Erase, Hash, HashAlgorithm, Keep, Mask, Replace,
};
use elide_core::entity::LabelCatalog;
use elide_core::modality::tabular::Tabular;
use elide_core::redaction::Attribution;
use elide_core::{Error, ErrorKind};
use nvisy_core::policy::redaction::{HashAlgorithm as PolicyHashAlgorithm, TabularRedaction};
use nvisy_core::policy::{Action, EntitySelector, Policy};

use super::selector::{attach, default_attribution, rule_attribution};

/// Compile every tabular-rule across `policies` into a
/// tabular-modality anonymizer.
pub fn compile_tabular(
    policies: &[Policy],
    catalog: LabelCatalog,
) -> Result<Anonymizer<Tabular>, Error> {
    let mut anonymizer = Anonymizer::<Tabular>::new().with_catalog(catalog);
    for policy in policies {
        for rule in &policy.rules {
            if !rule.enabled {
                continue;
            }
            let Action::Redact(redactions) = &rule.action else {
                continue;
            };
            let Some(spec) = &redactions.tabular else {
                continue;
            };
            anonymizer = attach_tabular(
                anonymizer,
                &rule.selector,
                spec,
                rule_attribution(policy, rule),
            )?;
        }
        if let Some(Action::Redact(redactions)) = &policy.default_action
            && let Some(spec) = &redactions.tabular
        {
            anonymizer = attach_tabular_fallback(anonymizer, spec, default_attribution(policy))?;
        }
    }
    Ok(anonymizer)
}

fn attach_tabular(
    anonymizer: Anonymizer<Tabular>,
    selector: &EntitySelector,
    spec: &TabularRedaction,
    attribution: Attribution,
) -> Result<Anonymizer<Tabular>, Error> {
    Ok(match build(spec)? {
        TabularOp::Erase => attach(anonymizer, selector, Erase, attribution),
        TabularOp::Keep => attach(anonymizer, selector, Keep, attribution),
        TabularOp::Mask(op) => attach(anonymizer, selector, op, attribution),
        TabularOp::Replace(op) => attach(anonymizer, selector, op, attribution),
        TabularOp::Hash(op) => attach(anonymizer, selector, op, attribution),
        TabularOp::DropRow => attach(anonymizer, selector, DropRow, attribution),
        TabularOp::DropColumn => attach(anonymizer, selector, DropColumn, attribution),
    })
}

fn attach_tabular_fallback(
    anonymizer: Anonymizer<Tabular>,
    spec: &TabularRedaction,
    attribution: Attribution,
) -> Result<Anonymizer<Tabular>, Error> {
    Ok(match build(spec)? {
        TabularOp::Erase => anonymizer.with_fallback(Erase).because(attribution),
        TabularOp::Keep => anonymizer.with_fallback(Keep).because(attribution),
        TabularOp::Mask(op) => anonymizer.with_fallback(op).because(attribution),
        TabularOp::Replace(op) => anonymizer.with_fallback(op).because(attribution),
        TabularOp::Hash(op) => anonymizer.with_fallback(op).because(attribution),
        TabularOp::DropRow => anonymizer.with_fallback(DropRow).because(attribution),
        TabularOp::DropColumn => anonymizer.with_fallback(DropColumn).because(attribution),
    })
}

enum TabularOp {
    Erase,
    Keep,
    Mask(Mask),
    Replace(Replace),
    Hash(Hash),
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
            let mut op = Hash::new(map_hash_algorithm(*algorithm));
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

fn map_hash_algorithm(spec: PolicyHashAlgorithm) -> HashAlgorithm {
    match spec {
        PolicyHashAlgorithm::Sha256 => HashAlgorithm::Sha256,
        PolicyHashAlgorithm::Sha512 => HashAlgorithm::Sha512,
    }
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
