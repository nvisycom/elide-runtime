//! Compile image-modality rules to elide operators + attach to an
//! [`Anonymizer<Image>`].

use elide::redaction::Anonymizer;
use elide::redaction::operators::{Blackbox, Blur, Erase, Keep, Pixelate};
use elide_core::modality::image::Image;
use nvisy_core::policy::redaction::ImageRedaction;
use nvisy_core::policy::{Policy, Rule, RuleAction};
use uuid::Uuid;

use super::selector::{attach, attach_override, fallback_attribution, rule_attribution};

/// Attach every image-applicable rule from `policies` onto an
/// already-constructed anonymizer. Takes an iterator so the
/// apply pipeline can pre-filter by [`Policy::applies_when`]
/// without cloning.
///
/// [`Policy::applies_when`]: nvisy_core::policy::Policy::applies_when
pub(crate) fn attach_policies_image<'a>(
    mut anonymizer: Anonymizer<Image>,
    policies: impl Iterator<Item = &'a Policy>,
) -> Anonymizer<Image> {
    for policy in policies {
        for rule in &policy.rules {
            let RuleAction::Redact(redactions) = &rule.action else {
                continue;
            };
            let Some(spec) = &redactions.image else {
                continue;
            };
            anonymizer = attach_rule(anonymizer, policy, rule, spec);
        }
        if let Some(RuleAction::Redact(redactions)) = &policy.fallback
            && let Some(spec) = &redactions.image
        {
            anonymizer = attach_fallback(anonymizer, policy, spec);
        }
    }
    anonymizer
}

/// Attach a reviewer override for one entity. No-op when the
/// override is not an image-modality `Redact`.
pub(crate) fn attach_override_image(
    anonymizer: Anonymizer<Image>,
    entity_id: Uuid,
    action: &RuleAction,
) -> Anonymizer<Image> {
    let RuleAction::Redact(redactions) = action else {
        return anonymizer;
    };
    let Some(spec) = &redactions.image else {
        return anonymizer;
    };
    match build(spec) {
        ImageOp::Erase => attach_override(anonymizer, entity_id, Erase),
        ImageOp::Keep => attach_override(anonymizer, entity_id, Keep),
        ImageOp::Blur(op) => attach_override(anonymizer, entity_id, op),
        ImageOp::Pixelate(op) => attach_override(anonymizer, entity_id, op),
        ImageOp::Blackbox(op) => attach_override(anonymizer, entity_id, op),
    }
}

fn attach_rule(
    anonymizer: Anonymizer<Image>,
    policy: &Policy,
    rule: &Rule,
    spec: &ImageRedaction,
) -> Anonymizer<Image> {
    let attribution = rule_attribution(policy, rule);
    match build(spec) {
        ImageOp::Erase => attach(anonymizer, &rule.predicate, Erase, attribution),
        ImageOp::Keep => attach(anonymizer, &rule.predicate, Keep, attribution),
        ImageOp::Blur(op) => attach(anonymizer, &rule.predicate, op, attribution),
        ImageOp::Pixelate(op) => attach(anonymizer, &rule.predicate, op, attribution),
        ImageOp::Blackbox(op) => attach(anonymizer, &rule.predicate, op, attribution),
    }
}

fn attach_fallback(
    anonymizer: Anonymizer<Image>,
    policy: &Policy,
    spec: &ImageRedaction,
) -> Anonymizer<Image> {
    let attribution = fallback_attribution(policy);
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
        ImageRedaction::Blackbox { color } => ImageOp::Blackbox(Blackbox::new(*color)),
    }
}
