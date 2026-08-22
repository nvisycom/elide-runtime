//! Compile audio-modality rules to elide operators + attach to an
//! [`Anonymizer<Audio>`].

use elide::Result;
use elide::modality::audio::Audio;
use elide::redaction::Anonymizer;
use elide_governance::PolicyDefinition;
use elide_governance::redaction::{AudioRedaction, ModalityRedactions};

use super::compile::{Target, attach_one_override, attach_policies};
use super::operator::audio::AudioOp;
use crate::entity::OverrideEntry;

/// Attach every audio-applicable rule from `policies` onto an
/// already-constructed anonymizer.
///
/// Takes an iterator so the apply pipeline can pre-filter by
/// [`PolicyDefinition::when`] without cloning.
///
/// [`PolicyDefinition::when`]: elide_governance::PolicyDefinition::when
pub(crate) fn attach_policies_audio<'a>(
    anonymizer: Anonymizer<Audio>,
    policies: impl Iterator<Item = &'a PolicyDefinition> + Clone,
) -> Result<Anonymizer<Audio>> {
    attach_policies(anonymizer, policies, compile_one)
}

/// Attach a reviewer override for one entity. Always attaches:
/// the entry's spec is typed to this modality, so there is no
/// absent-arm case to fall through.
pub(crate) fn attach_override_audio(
    anonymizer: Anonymizer<Audio>,
    entry: &OverrideEntry<Audio>,
) -> Result<Anonymizer<Audio>> {
    attach_one_override(anonymizer, entry, compile_spec)
}

fn compile_one(
    target: Target<'_, Audio>,
    redactions: &ModalityRedactions,
) -> Result<Anonymizer<Audio>> {
    let Some(spec) = &redactions.audio else {
        return Ok(target.passthrough());
    };
    compile_spec(target, spec)
}

fn compile_spec(target: Target<'_, Audio>, spec: &AudioRedaction) -> Result<Anonymizer<Audio>> {
    Ok(AudioOp::from(spec).attach_to(target))
}
