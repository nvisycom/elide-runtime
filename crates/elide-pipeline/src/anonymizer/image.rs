//! Compile image-modality rules to elide operators + attach to an
//! [`Anonymizer<Image>`].

use elide::redaction::Anonymizer;
use elide_core::Result;
use elide_core::modality::image::Image;
use elide_governance::PolicyDefinition;
use elide_governance::redaction::ModalityRedactions;

use super::compile::{Target, attach_one_override, attach_policies};
use super::operator::image::ImageOp;
use crate::entity::OverrideEntry;

/// Attach every image-applicable rule from `policies` onto an
/// already-constructed anonymizer.
///
/// Takes an iterator so the apply pipeline can pre-filter by
/// [`PolicyDefinition::when`] without cloning.
///
/// [`PolicyDefinition::when`]: elide_governance::PolicyDefinition::when
pub(crate) fn attach_policies_image<'a>(
    anonymizer: Anonymizer<Image>,
    policies: impl Iterator<Item = &'a PolicyDefinition> + Clone,
) -> Result<Anonymizer<Image>> {
    attach_policies(anonymizer, policies, compile_one)
}

/// Attach a reviewer override for one entity. A no-op when the
/// override's redaction spec carries no image arm.
pub(crate) fn attach_override_image(
    anonymizer: Anonymizer<Image>,
    entry: &OverrideEntry,
) -> Result<Anonymizer<Image>> {
    attach_one_override(anonymizer, entry, compile_one)
}

fn compile_one(
    target: Target<'_, Image>,
    redactions: &ModalityRedactions,
) -> Result<Anonymizer<Image>> {
    let Some(spec) = &redactions.image else {
        return Ok(target.passthrough());
    };
    Ok(ImageOp::from(spec).attach_to(target))
}
