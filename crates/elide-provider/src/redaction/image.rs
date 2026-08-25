//! Compile image-modality rules to elide operators + attach to an
//! [`Anonymizer<Image>`].

use elide::Result;
use elide::modality::image::Image;
use elide::redaction::Anonymizer;
use elide_governance::PolicyDefinition;
use elide_governance::redaction::{ImageRedaction, ModalityRedactions};

use super::compile::{Target, attach_policies};
use super::operator::image::ImageOp;

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

fn compile_one(
    target: Target<'_, Image>,
    redactions: &ModalityRedactions,
) -> Result<Anonymizer<Image>> {
    let Some(spec) = &redactions.image else {
        return Ok(target.passthrough());
    };
    compile_spec(target, spec)
}

fn compile_spec(target: Target<'_, Image>, spec: &ImageRedaction) -> Result<Anonymizer<Image>> {
    Ok(ImageOp::from(spec).attach_to(target))
}
