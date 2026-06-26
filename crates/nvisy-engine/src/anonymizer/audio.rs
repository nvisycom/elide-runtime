//! Compile audio-modality rules to elide operators + attach to an
//! [`Anonymizer<Audio>`].

use elide::redaction::Anonymizer;
use elide::redaction::operators::{Beep, Erase, Keep, Silence};
use elide_core::Error;
use elide_core::entity::LabelCatalog;
use elide_core::modality::audio::Audio;
use nvisy_core::policy::redaction::AudioRedaction;
use nvisy_core::policy::{Policy, Rule, RuleAction};
use uuid::Uuid;

use super::selector::{attach, attach_override, fallback_attribution, rule_attribution};

/// Compile every audio-applicable rule across `policies` into an
/// audio-modality anonymizer.
pub fn compile_audio(
    policies: &[Policy],
    catalog: LabelCatalog,
) -> Result<Anonymizer<Audio>, Error> {
    let anonymizer = Anonymizer::<Audio>::new().with_catalog(catalog);
    Ok(attach_policies_audio(anonymizer, policies.iter()))
}

/// Attach every audio-applicable rule from `policies` onto an
/// already-constructed anonymizer. Takes an iterator so the
/// apply pipeline can pre-filter by [`Policy::applies_when`]
/// without cloning.
///
/// [`Policy::applies_when`]: nvisy_core::policy::Policy::applies_when
pub(crate) fn attach_policies_audio<'a>(
    mut anonymizer: Anonymizer<Audio>,
    policies: impl Iterator<Item = &'a Policy>,
) -> Anonymizer<Audio> {
    for policy in policies {
        for rule in &policy.rules {
            let RuleAction::Redact(redactions) = &rule.action else {
                continue;
            };
            let Some(spec) = &redactions.audio else {
                continue;
            };
            anonymizer = attach_rule(anonymizer, policy, rule, spec);
        }
        if let Some(RuleAction::Redact(redactions)) = &policy.fallback
            && let Some(spec) = &redactions.audio
        {
            anonymizer = attach_fallback(anonymizer, policy, spec);
        }
    }
    anonymizer
}

/// Attach a reviewer override for one entity. No-op when the
/// override is not an audio-modality `Redact`.
pub(crate) fn attach_override_audio(
    anonymizer: Anonymizer<Audio>,
    entity_id: Uuid,
    action: &RuleAction,
) -> Anonymizer<Audio> {
    let RuleAction::Redact(redactions) = action else {
        return anonymizer;
    };
    let Some(spec) = &redactions.audio else {
        return anonymizer;
    };
    match build(spec) {
        AudioOp::Erase => attach_override(anonymizer, entity_id, Erase),
        AudioOp::Keep => attach_override(anonymizer, entity_id, Keep),
        AudioOp::Silence => attach_override(anonymizer, entity_id, Silence),
        AudioOp::Beep(op) => attach_override(anonymizer, entity_id, op),
    }
}

fn attach_rule(
    anonymizer: Anonymizer<Audio>,
    policy: &Policy,
    rule: &Rule,
    spec: &AudioRedaction,
) -> Anonymizer<Audio> {
    let attribution = rule_attribution(policy, rule);
    match build(spec) {
        AudioOp::Erase => attach(anonymizer, &rule.predicate, Erase, attribution),
        AudioOp::Keep => attach(anonymizer, &rule.predicate, Keep, attribution),
        AudioOp::Silence => attach(anonymizer, &rule.predicate, Silence, attribution),
        AudioOp::Beep(op) => attach(anonymizer, &rule.predicate, op, attribution),
    }
}

fn attach_fallback(
    anonymizer: Anonymizer<Audio>,
    policy: &Policy,
    spec: &AudioRedaction,
) -> Anonymizer<Audio> {
    let attribution = fallback_attribution(policy);
    match build(spec) {
        AudioOp::Erase => anonymizer.with_fallback(Erase).because(attribution),
        AudioOp::Keep => anonymizer.with_fallback(Keep).because(attribution),
        AudioOp::Silence => anonymizer.with_fallback(Silence).because(attribution),
        AudioOp::Beep(op) => anonymizer.with_fallback(op).because(attribution),
    }
}

enum AudioOp {
    Erase,
    Keep,
    Silence,
    Beep(Beep),
}

fn build(spec: &AudioRedaction) -> AudioOp {
    match spec {
        AudioRedaction::Erase => AudioOp::Erase,
        AudioRedaction::Keep => AudioOp::Keep,
        AudioRedaction::Silence => AudioOp::Silence,
        AudioRedaction::Beep {
            hz,
            amplitude,
            waveform,
        } => AudioOp::Beep(
            Beep::new(*hz)
                .with_amplitude(*amplitude)
                .with_waveform((*waveform).into()),
        ),
    }
}
