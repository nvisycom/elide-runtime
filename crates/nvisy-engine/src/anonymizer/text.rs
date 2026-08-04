//! Compile text-modality rules to elide operators + attach to an
//! [`Anonymizer<Text>`].

use elide::redaction::Anonymizer;
use elide_core::Error;
use elide_core::modality::text::Text;
use nvisy_schema::policy::PolicyDefinition;
use nvisy_schema::policy::redaction::ModalityRedactions;
use uuid::Uuid;

use super::compile::{Target, attach_one_override, attach_policies};
use super::operator::text::TextOp;

/// Attach every text-applicable rule from `policies` onto an
/// already-constructed anonymizer. The apply pipeline calls this
/// after layering per-entity override rules so the overrides
/// keep first-match precedence. Takes an iterator (not a slice)
/// so callers can pre-filter by [`PolicyDefinition::when`] without
/// cloning the policy set.
///
/// [`PolicyDefinition::when`]: nvisy_schema::policy::PolicyDefinition::when
pub(crate) fn attach_policies_text<'a>(
    anonymizer: Anonymizer<Text>,
    policies: impl Iterator<Item = &'a PolicyDefinition>,
) -> Result<Anonymizer<Text>, Error> {
    attach_policies(anonymizer, policies, compile_one)
}

/// Attach a reviewer override for one entity. A no-op when the
/// override's redaction spec carries no text arm.
pub(crate) fn attach_override_text(
    anonymizer: Anonymizer<Text>,
    entity_id: Uuid,
    redactions: &ModalityRedactions,
) -> Result<Anonymizer<Text>, Error> {
    attach_one_override(anonymizer, entity_id, redactions, compile_one)
}

fn compile_one(
    target: Target<'_, Text>,
    redactions: &ModalityRedactions,
) -> Result<Anonymizer<Text>, Error> {
    let Some(spec) = &redactions.text else {
        return Ok(target.passthrough());
    };
    Ok(TextOp::try_from(spec)?.attach_to(target))
}
