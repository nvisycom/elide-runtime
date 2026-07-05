//! Compile audio-modality rules to elide operators + attach to an
//! [`Anonymizer<Audio>`].

use elide::redaction::Anonymizer;
use elide::redaction::operators::{Beep, Erase, Keep, Silence};
use elide_core::Error;
use elide_core::modality::audio::Audio;
use nvisy_schema::policy::PolicyAction;
use nvisy_schema::policy::redaction::{AudioRedaction, ModalityRedactions};
use uuid::Uuid;

use super::dispatch::{Target, attach_one_override, attach_policies};

/// Attach every audio-applicable rule from `policies` onto an
/// already-constructed anonymizer. Takes an iterator so the
/// apply pipeline can pre-filter by [`Policy::applies_when`]
/// without cloning.
///
/// [`Policy::applies_when`]: nvisy_schema::policy::Policy::applies_when
pub(crate) fn attach_policies_audio<'a>(
    anonymizer: Anonymizer<Audio>,
    policies: impl Iterator<Item = &'a nvisy_schema::policy::Policy>,
) -> Result<Anonymizer<Audio>, Error> {
    attach_policies(anonymizer, policies, compile_one)
}

/// Attach a reviewer override for one entity. No-op when the
/// override is not an audio-modality `Redact`.
pub(crate) fn attach_override_audio(
    anonymizer: Anonymizer<Audio>,
    entity_id: Uuid,
    action: &PolicyAction,
) -> Result<Anonymizer<Audio>, Error> {
    attach_one_override(anonymizer, entity_id, action, compile_one)
}

fn compile_one(
    target: Target<'_, Audio>,
    redactions: &ModalityRedactions,
) -> Result<Anonymizer<Audio>, Error> {
    let Some(spec) = &redactions.audio else {
        return Ok(target.passthrough());
    };
    Ok(match build(spec) {
        AudioOp::Erase => target.attach_with(Erase),
        AudioOp::Keep => target.attach_with(Keep),
        AudioOp::Silence => target.attach_with(Silence),
        AudioOp::Beep(op) => target.attach_with(op),
    })
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
                .with_waveform(*waveform),
        ),
    }
}
