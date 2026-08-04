//! Compile audio-modality rules to elide operators + attach to an
//! [`Anonymizer<Audio>`].

use elide::redaction::Anonymizer;
use elide_core::Error;
use elide_core::modality::audio::Audio;
use nvisy_schema::policy::redaction::ModalityRedactions;
use nvisy_schema::policy::{PolicyDefinition, PolicyAction};
use uuid::Uuid;

use super::compile::{Target, attach_one_override, attach_policies};
use super::operator::audio::AudioOp;

/// Attach every audio-applicable rule from `policies` onto an
/// already-constructed anonymizer.
///
/// Takes an iterator so the apply pipeline can pre-filter by
/// [`PolicyDefinition::when`] without cloning.
///
/// [`PolicyDefinition::when`]: nvisy_schema::policy::PolicyDefinition::when
pub(crate) fn attach_policies_audio<'a>(
    anonymizer: Anonymizer<Audio>,
    policies: impl Iterator<Item = &'a PolicyDefinition>,
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
    Ok(AudioOp::from(spec).attach_to(target))
}
