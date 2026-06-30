//! Compile image-modality rules to elide operators + attach to an
//! [`Anonymizer<Image>`].

use elide::redaction::Anonymizer;
use elide::redaction::operators::{Blackbox, Blur, Erase, Keep, Pixelate};
use elide_core::Error;
use elide_core::modality::image::Image;
use nvisy_core::policy::RuleAction;
use nvisy_core::policy::redaction::{ImageRedaction, ModalityRedactions};
use uuid::Uuid;

use super::dispatch::{Target, attach_one_override, attach_policies};

/// Attach every image-applicable rule from `policies` onto an
/// already-constructed anonymizer. Takes an iterator so the
/// apply pipeline can pre-filter by [`Policy::applies_when`]
/// without cloning.
///
/// [`Policy::applies_when`]: nvisy_core::policy::Policy::applies_when
pub(crate) fn attach_policies_image<'a>(
    anonymizer: Anonymizer<Image>,
    policies: impl Iterator<Item = &'a nvisy_core::policy::Policy>,
) -> Result<Anonymizer<Image>, Error> {
    attach_policies(anonymizer, policies, compile_one)
}

/// Attach a reviewer override for one entity. No-op when the
/// override is not an image-modality `Redact`.
pub(crate) fn attach_override_image(
    anonymizer: Anonymizer<Image>,
    entity_id: Uuid,
    action: &RuleAction,
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
    Ok(match build(spec) {
        ImageOp::Erase => target.attach_with(Erase),
        ImageOp::Keep => target.attach_with(Keep),
        ImageOp::Blur(op) => target.attach_with(op),
        ImageOp::Pixelate(op) => target.attach_with(op),
        ImageOp::Blackbox(op) => target.attach_with(op),
    })
}

enum ImageOp {
    Erase,
    Keep,
    Blur(Blur),
    Pixelate(Pixelate),
    Blackbox(Blackbox),
}

fn build(spec: &ImageRedaction) -> ImageOp {
    match spec {
        ImageRedaction::Erase => ImageOp::Erase,
        ImageRedaction::Keep => ImageOp::Keep,
        ImageRedaction::Blur { sigma } => ImageOp::Blur(Blur::new(*sigma)),
        ImageRedaction::Pixelate { block_size } => ImageOp::Pixelate(Pixelate::new(*block_size)),
        ImageRedaction::Blackbox { color } => ImageOp::Blackbox(Blackbox::new(*color)),
    }
}
