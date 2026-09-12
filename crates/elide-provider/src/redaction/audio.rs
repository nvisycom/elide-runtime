//! Compile audio-modality rules to elide operators + attach to an
//! [`Anonymizer<Audio>`].

use elide::Result;
use elide::modality::audio::Audio;
use elide::redaction::Anonymizer;
use elide_governance::policy::Policy;
use elide_governance::redaction::{AudioRedaction, ModalityRedactions};

use super::compile::{Target, attach_policies};
use super::operator::audio::AudioOp;

/// Attach every audio-applicable rule from `policies` onto an
/// already-constructed anonymizer.
///
/// Takes an iterator so the apply pipeline can pre-filter by
/// [`Policy::when`] without cloning.
///
/// [`Policy::when`]: elide_governance::policy::Policy::when
pub(crate) fn attach_policies_audio<'a>(
    anonymizer: Anonymizer<Audio>,
    policies: impl Iterator<Item = &'a Policy> + Clone,
) -> Result<Anonymizer<Audio>> {
    attach_policies(anonymizer, policies, compile_one)
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
