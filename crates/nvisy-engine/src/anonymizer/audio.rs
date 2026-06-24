//! Compile [`AudioRedaction`] specs to elide audio operators.

use elide::Anonymizer;
use elide::redaction::operators::{Beep, Erase, Keep, Silence};
use elide_core::Error;
use elide_core::entity::LabelCatalog;
use elide_core::modality::audio::Audio;
use elide_core::redaction::Attribution;
use nvisy_core::policy::redaction::AudioRedaction;
use nvisy_core::policy::{Action, EntitySelector, Policy};

use super::selector::{attach, default_attribution, rule_attribution};

/// Compile every audio-rule across `policies` into an audio-modality
/// anonymizer.
pub fn compile_audio(
    policies: &[Policy],
    catalog: LabelCatalog,
) -> Result<Anonymizer<Audio>, Error> {
    let mut anonymizer = Anonymizer::<Audio>::new().with_catalog(catalog);
    for policy in policies {
        for rule in &policy.rules {
            if !rule.enabled {
                continue;
            }
            let Action::Redact(redactions) = &rule.action else {
                continue;
            };
            let Some(spec) = &redactions.audio else {
                continue;
            };
            anonymizer = attach_audio(
                anonymizer,
                &rule.selector,
                spec,
                rule_attribution(policy, rule),
            );
        }
        if let Some(Action::Redact(redactions)) = &policy.default_action
            && let Some(spec) = &redactions.audio
        {
            anonymizer = attach_audio_fallback(anonymizer, spec, default_attribution(policy));
        }
    }
    Ok(anonymizer)
}

fn attach_audio(
    anonymizer: Anonymizer<Audio>,
    selector: &EntitySelector,
    spec: &AudioRedaction,
    attribution: Attribution,
) -> Anonymizer<Audio> {
    match build(spec) {
        AudioOp::Erase => attach(anonymizer, selector, Erase, attribution),
        AudioOp::Keep => attach(anonymizer, selector, Keep, attribution),
        AudioOp::Silence => attach(anonymizer, selector, Silence, attribution),
        AudioOp::Beep(op) => attach(anonymizer, selector, op, attribution),
    }
}

fn attach_audio_fallback(
    anonymizer: Anonymizer<Audio>,
    spec: &AudioRedaction,
    attribution: Attribution,
) -> Anonymizer<Audio> {
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
