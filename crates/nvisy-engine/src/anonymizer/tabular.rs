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
use elide_core::Error;
use elide_core::modality::tabular::Tabular;
use nvisy_schema::policy::PolicyAction;
use nvisy_schema::policy::redaction::{ModalityRedactions, TabularRedaction};
use uuid::Uuid;

use super::compile::{Target, attach_one_override, attach_policies};
use super::operator::text::TextOp;

/// Attach every tabular-applicable rule from `policies` onto an
/// already-constructed anonymizer. Takes an iterator so the
/// apply pipeline can pre-filter by [`Policy::applies_when`]
/// without cloning.
///
/// [`Policy::applies_when`]: nvisy_schema::policy::Policy::applies_when
pub(crate) fn attach_policies_tabular<'a>(
    anonymizer: Anonymizer<Tabular>,
    policies: impl Iterator<Item = &'a nvisy_schema::policy::Policy>,
) -> Result<Anonymizer<Tabular>, Error> {
    attach_policies(anonymizer, policies, compile_one)
}

/// Attach a reviewer override for one entity. No-op when the
/// override is not a tabular-modality `Redact`.
pub(crate) fn attach_override_tabular(
    anonymizer: Anonymizer<Tabular>,
    entity_id: Uuid,
    action: &PolicyAction,
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
    Ok(match spec {
        TabularRedaction::Cell { spec } => TextOp::try_from(spec)?.attach_to(target),
        TabularRedaction::DropRow => target.attach_with(DropRow),
        TabularRedaction::DropColumn => target.attach_with(DropColumn),
    })
}
