//! Compile tabular-modality rules to elide operators + attach to
//! an [`Anonymizer<Tabular>`].
//!
//! Tabular cells are `TextBacked` in elide, so cell-level ops
//! share the [`super::operator::text`] builder with the text
//! modality. The two structural operators: [`DropRow`] and
//! [`DropColumn`]: sit alongside as tabular-only.
//!
//! [`DropRow`]: elide::redaction::operators::DropRow
//! [`DropColumn`]: elide::redaction::operators::DropColumn

use elide::Result;
use elide::modality::tabular::Tabular;
use elide::redaction::Anonymizer;
use elide::redaction::operators::{DropColumn, DropRow};
use elide_governance::PolicyDefinition;
use elide_governance::redaction::{ModalityRedactions, TabularRedaction};

use super::compile::{Target, attach_one_override, attach_policies};
use super::operator::text::{TextOperatorContext, compile_and_attach};
use crate::entity::OverrideEntry;

/// Attach every tabular-applicable rule from `policies` onto an
/// already-constructed anonymizer. Takes an iterator so the
/// apply pipeline can pre-filter by [`PolicyDefinition::when`]
/// without cloning.
///
/// [`PolicyDefinition::when`]: elide_governance::PolicyDefinition::when
pub(crate) fn attach_policies_tabular<'a>(
    anonymizer: Anonymizer<Tabular>,
    policies: impl Iterator<Item = &'a PolicyDefinition> + Clone,
    ctx: &TextOperatorContext,
) -> Result<Anonymizer<Tabular>> {
    attach_policies(anonymizer, policies, |target, redactions| {
        compile_one(target, redactions, ctx)
    })
}

/// Attach a reviewer override for one entity. Always attaches:
/// the entry's spec is typed to this modality, so there is no
/// absent-arm case to fall through.
pub(crate) fn attach_override_tabular(
    anonymizer: Anonymizer<Tabular>,
    entry: &OverrideEntry<Tabular>,
    ctx: &TextOperatorContext,
) -> Result<Anonymizer<Tabular>> {
    attach_one_override(anonymizer, entry, |target, spec| {
        compile_spec(target, spec, ctx)
    })
}

fn compile_one(
    target: Target<'_, Tabular>,
    redactions: &ModalityRedactions,
    ctx: &TextOperatorContext,
) -> Result<Anonymizer<Tabular>> {
    match &redactions.tabular {
        None => Ok(target.passthrough()),
        Some(spec) => compile_spec(target, spec, ctx),
    }
}

fn compile_spec(
    target: Target<'_, Tabular>,
    spec: &TabularRedaction,
    ctx: &TextOperatorContext,
) -> Result<Anonymizer<Tabular>> {
    match spec {
        TabularRedaction::Cell { spec } => compile_and_attach(spec, ctx, target),
        TabularRedaction::DropRow => Ok(target.attach_with(DropRow)),
        TabularRedaction::DropColumn => Ok(target.attach_with(DropColumn)),
    }
}
