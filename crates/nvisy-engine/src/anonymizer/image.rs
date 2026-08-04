//! Compile image-modality rules to elide operators + attach to an
//! [`Anonymizer<Image>`].

use elide::redaction::Anonymizer;
use elide_core::Error;
use elide_core::modality::image::Image;
use nvisy_schema::policy::redaction::ModalityRedactions;
use nvisy_schema::policy::{PolicyDefinition, PolicyAction};
use uuid::Uuid;

use super::compile::{Target, attach_one_override, attach_policies};
use super::operator::image::ImageOp;

/// Attach every image-applicable rule from `policies` onto an
/// already-constructed anonymizer.
///
/// Takes an iterator so the apply pipeline can pre-filter by
/// [`PolicyDefinition::when`] without cloning.
///
/// [`PolicyDefinition::when`]: nvisy_schema::policy::PolicyDefinition::when
pub(crate) fn attach_policies_image<'a>(
    anonymizer: Anonymizer<Image>,
    policies: impl Iterator<Item = &'a PolicyDefinition>,
) -> Result<Anonymizer<Image>, Error> {
    attach_policies(anonymizer, policies, compile_one)
}

/// Attach a reviewer override for one entity. No-op when the
/// override is not an image-modality `Redact`.
pub(crate) fn attach_override_image(
    anonymizer: Anonymizer<Image>,
    entity_id: Uuid,
    action: &PolicyAction,
) -> Result<Anonymizer<Image>, Error> {
    attach_one_override(anonymizer, entity_id, action, compile_one)
}

fn compile_one(
    target: Target<'_, Image>,
    redactions: &ModalityRedactions,
) -> Result<Anonymizer<Image>, Error> {
    let Some(spec) = &redactions.image else {
        return Ok(target.passthrough());
    };
    Ok(ImageOp::from(spec).attach_to(target))
}
