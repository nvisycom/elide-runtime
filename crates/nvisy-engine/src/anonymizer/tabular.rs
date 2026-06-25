//! Compile tabular-modality rules to elide operators + attach to
//! an [`Anonymizer<Tabular>`].
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
use elide_core::{Error, ErrorKind};
use nvisy_core::policy::redaction::{HashAlgorithm as PolicyHashAlgorithm, TabularRedaction};
use nvisy_core::policy::{Policy, Rule, RuleAction};
use uuid::Uuid;

use super::selector::{attach, attach_override, fallback_attribution, rule_attribution};

/// Compile every tabular-applicable rule across `policies` into a
/// tabular-modality anonymizer.
pub fn compile_tabular(
    policies: &[Policy],
    catalog: LabelCatalog,
) -> Result<Anonymizer<Tabular>, Error> {
    let anonymizer = Anonymizer::<Tabular>::new().with_catalog(catalog);
    attach_policies_tabular(anonymizer, policies.iter())
}

/// Attach every tabular-applicable rule from `policies` onto an
/// already-constructed anonymizer. Takes an iterator so the
/// apply pipeline can pre-filter by [`Policy::applies_when`]
/// without cloning.
///
/// [`Policy::applies_when`]: nvisy_core::policy::Policy::applies_when
pub(crate) fn attach_policies_tabular<'a>(
    mut anonymizer: Anonymizer<Tabular>,
    policies: impl Iterator<Item = &'a Policy>,
) -> Result<Anonymizer<Tabular>, Error> {
    for policy in policies {
        for rule in &policy.rules {
            let RuleAction::Redact(redactions) = &rule.action else {
                continue;
            };
            let Some(spec) = &redactions.tabular else {
                continue;
            };
            anonymizer = attach_rule(anonymizer, policy, rule, spec)?;
        }
        if let Some(RuleAction::Redact(redactions)) = &policy.fallback
            && let Some(spec) = &redactions.tabular
        {
            anonymizer = attach_fallback(anonymizer, policy, spec)?;
        }
    }
    Ok(anonymizer)
}

/// Attach a reviewer override for one entity. No-op when the
/// override is not a tabular-modality `Redact`.
pub(crate) fn attach_override_tabular(
    anonymizer: Anonymizer<Tabular>,
    entity_id: Uuid,
    action: &RuleAction,
) -> Result<Anonymizer<Tabular>, Error> {
    let RuleAction::Redact(redactions) = action else {
        return Ok(anonymizer);
    };
    let Some(spec) = &redactions.tabular else {
        return Ok(anonymizer);
    };
    Ok(match build(spec)? {
        TabularOp::Erase => attach_override(anonymizer, entity_id, Erase),
        TabularOp::Keep => attach_override(anonymizer, entity_id, Keep),
        TabularOp::Mask(op) => attach_override(anonymizer, entity_id, op),
        TabularOp::Replace(op) => attach_override(anonymizer, entity_id, op),
        TabularOp::Hash(op) => attach_override(anonymizer, entity_id, op),
        TabularOp::DropRow => attach_override(anonymizer, entity_id, DropRow),
        TabularOp::DropColumn => attach_override(anonymizer, entity_id, DropColumn),
    })
}

fn attach_rule(
    anonymizer: Anonymizer<Tabular>,
    policy: &Policy,
    rule: &Rule,
    spec: &TabularRedaction,
) -> Result<Anonymizer<Tabular>, Error> {
    let attribution = rule_attribution(policy, rule);
    Ok(match build(spec)? {
        TabularOp::Erase => attach(anonymizer, &rule.predicate, Erase, attribution),
        TabularOp::Keep => attach(anonymizer, &rule.predicate, Keep, attribution),
        TabularOp::Mask(op) => attach(anonymizer, &rule.predicate, op, attribution),
        TabularOp::Replace(op) => attach(anonymizer, &rule.predicate, op, attribution),
        TabularOp::Hash(op) => attach(anonymizer, &rule.predicate, op, attribution),
        TabularOp::DropRow => attach(anonymizer, &rule.predicate, DropRow, attribution),
        TabularOp::DropColumn => attach(anonymizer, &rule.predicate, DropColumn, attribution),
    })
}

fn attach_fallback(
    anonymizer: Anonymizer<Tabular>,
    policy: &Policy,
    spec: &TabularRedaction,
) -> Result<Anonymizer<Tabular>, Error> {
    let attribution = fallback_attribution(policy);
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
