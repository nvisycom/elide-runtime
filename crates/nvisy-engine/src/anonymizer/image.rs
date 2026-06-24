//! Compile [`ImageRedaction`] specs to elide image operators.

use elide::Anonymizer;
use elide::redaction::operators::{Blackbox, Blur, Erase, Keep, Pixelate};
use elide_core::Error;
use elide_core::entity::LabelCatalog;
use elide_core::modality::image::Image;
use elide_core::redaction::Attribution;
use nvisy_core::policy::redaction::ImageRedaction;
use nvisy_core::policy::{Action, EntitySelector, Policy};

use super::selector::{attach, default_attribution, rule_attribution};

/// Compile every image-rule across `policies` into an image-modality
/// anonymizer.
pub fn compile_image(
    policies: &[Policy],
    catalog: LabelCatalog,
) -> Result<Anonymizer<Image>, Error> {
    let mut anonymizer = Anonymizer::<Image>::new().with_catalog(catalog);
    for policy in policies {
        for rule in &policy.rules {
            if !rule.enabled {
                continue;
            }
            let Action::Redact(redactions) = &rule.action else {
                continue;
            };
            let Some(spec) = &redactions.image else {
                continue;
            };
            anonymizer = attach_image(
                anonymizer,
                &rule.selector,
                spec,
                rule_attribution(policy, rule),
            );
        }
        if let Some(Action::Redact(redactions)) = &policy.default_action
            && let Some(spec) = &redactions.image
        {
            anonymizer = attach_image_fallback(anonymizer, spec, default_attribution(policy));
        }
    }
    Ok(anonymizer)
}

fn attach_image(
    anonymizer: Anonymizer<Image>,
    selector: &EntitySelector,
    spec: &ImageRedaction,
    attribution: Attribution,
) -> Anonymizer<Image> {
    match build(spec) {
        ImageOp::Erase => attach(anonymizer, selector, Erase, attribution),
        ImageOp::Keep => attach(anonymizer, selector, Keep, attribution),
        ImageOp::Blur(op) => attach(anonymizer, selector, op, attribution),
        ImageOp::Pixelate(op) => attach(anonymizer, selector, op, attribution),
        ImageOp::Blackbox(op) => attach(anonymizer, selector, op, attribution),
    }
}

fn attach_image_fallback(
    anonymizer: Anonymizer<Image>,
    spec: &ImageRedaction,
    attribution: Attribution,
) -> Anonymizer<Image> {
    match build(spec) {
        ImageOp::Erase => anonymizer.with_fallback(Erase).because(attribution),
        ImageOp::Keep => anonymizer.with_fallback(Keep).because(attribution),
        ImageOp::Blur(op) => anonymizer.with_fallback(op).because(attribution),
        ImageOp::Pixelate(op) => anonymizer.with_fallback(op).because(attribution),
        ImageOp::Blackbox(op) => anonymizer.with_fallback(op).because(attribution),
    }
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
        ImageRedaction::Blackbox { color } => ImageOp::Blackbox(Blackbox::new((*color).into())),
    }
}
