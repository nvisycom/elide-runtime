//! Compile image-modality rules to elide operators + attach to an
//! [`Anonymizer<Image>`].

use elide::modality::image::Image;
use elide::redaction::Anonymizer;
use elide::{Error, ErrorKind, Result};
use elide_schema::policy::Policy;
use elide_schema::redaction::{ImageRedaction, ModalityRedactions};

use super::operator::image::ImageOp;
use super::target::{Target, attach_policies};

/// Attach every image-applicable rule from `policies` onto an
/// already-constructed anonymizer.
///
/// Takes an iterator so the apply pipeline can pre-filter by
/// [`Policy::when`] without cloning.
///
/// [`Policy::when`]: elide_schema::policy::Policy::when
pub(crate) fn attach_policies_image<'a>(
    anonymizer: Anonymizer<Image>,
    policies: impl Iterator<Item = &'a Policy> + Clone,
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
    // Named here rather than in elide's message, which knows the
    // parameter but not which policy set it.
    let policy_id = target.policy_id();
    let op = ImageOp::try_from(spec).map_err(|err| {
        Error::new(
            ErrorKind::Configuration,
            format!("policy `{policy_id}`: {err}"),
        )
    })?;
    Ok(op.attach_to(target))
}
