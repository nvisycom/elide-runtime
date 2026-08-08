//! Compile tabular-modality rules to elide operators + attach to
//! an [`Anonymizer<Tabular>`].
//!
//! Tabular cells are `TextBacked` in elide, so cell-level ops
//! share the [`super::operator::text`] builder with the text
//! modality. The two structural operators — [`DropRow`] and
//! [`DropColumn`] — sit alongside as tabular-only.
//!
//! [`DropRow`]: elide::redaction::operators::DropRow
//! [`DropColumn`]: elide::redaction::operators::DropColumn

use elide::redaction::Anonymizer;
use elide::redaction::operators::{DropColumn, DropRow};
use elide_core::Result;
use elide_core::modality::tabular::Tabular;
use nvisy_schema::policy::PolicyDefinition;
use nvisy_schema::policy::redaction::{ModalityRedactions, TabularRedaction};
use uuid::Uuid;

use super::compile::{Target, attach_one_override, attach_policies};
use super::operator::text::{TextOperatorContext, compile_and_attach};

/// Attach every tabular-applicable rule from `policies` onto an
/// already-constructed anonymizer. Takes an iterator so the
/// apply pipeline can pre-filter by [`PolicyDefinition::when`]
/// without cloning.
///
/// [`PolicyDefinition::when`]: nvisy_schema::policy::PolicyDefinition::when
pub(crate) fn attach_policies_tabular<'a>(
    anonymizer: Anonymizer<Tabular>,
    policies: impl Iterator<Item = &'a PolicyDefinition>,
    ctx: &TextOperatorContext,
) -> Result<Anonymizer<Tabular>> {
    attach_policies(anonymizer, policies, |target, redactions| {
        compile_one(target, redactions, ctx)
    })
}

/// Attach a reviewer override for one entity. A no-op when the
/// override's redaction spec carries no tabular arm.
pub(crate) fn attach_override_tabular(
    anonymizer: Anonymizer<Tabular>,
    entity_id: Uuid,
    redactions: &ModalityRedactions,
    ctx: &TextOperatorContext,
) -> Result<Anonymizer<Tabular>> {
    attach_one_override(anonymizer, entity_id, redactions, |target, redactions| {
        compile_one(target, redactions, ctx)
    })
}

fn compile_one(
    target: Target<'_, Tabular>,
    redactions: &ModalityRedactions,
    ctx: &TextOperatorContext,
) -> Result<Anonymizer<Tabular>> {
    match &redactions.tabular {
        None => Ok(target.passthrough()),
        Some(TabularRedaction::Cell { spec }) => compile_and_attach(spec, ctx, target),
        Some(TabularRedaction::DropRow) => Ok(target.attach_with(DropRow)),
        Some(TabularRedaction::DropColumn) => Ok(target.attach_with(DropColumn)),
    }
}
