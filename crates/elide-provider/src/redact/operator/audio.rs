//! Audio-modality operator builder.
//!
//! Symmetric with [`super::text`] and [`super::image`]: builds
//! an [`AudioOp`] from an `AudioRedaction` spec, dispatches it
//! onto a [`Target`] with the right concrete elide operator
//! type. Audio operators do not cross modalities.

use elide::modality::audio::Audio;
use elide::redaction::Anonymizer;
use elide::redaction::operators::{Beep, Cover, Keep, Remove};
use elide::{Error, Result};
use elide_schema::redaction::AudioRedaction;

use crate::redact::target::Target;

/// Discriminated builder result so [`Target::attach_with`] can
/// attach the right concrete operator type. Same reason as
/// [`super::text::TextOp`]: [`Anonymizer::with_label`] takes
/// `O: Operator<M> + 'static` by value.
///
/// [`Anonymizer::with_label`]: elide::redaction::Anonymizer::with_label
pub(in crate::redact) enum AudioOp {
    Remove,
    Keep,
    Cover,
    Beep(Beep),
}

impl AudioOp {
    /// Attach `self` to `target`.
    pub(in crate::redact) fn attach_to(self, target: Target<'_, Audio>) -> Anonymizer<Audio> {
        match self {
            AudioOp::Remove => target.attach_with(Remove),
            AudioOp::Keep => target.attach_with(Keep),
            AudioOp::Cover => target.attach_with(Cover::default()),
            AudioOp::Beep(op) => target.attach_with(op),
        }
    }
}

impl TryFrom<&AudioRedaction> for AudioOp {
    type Error = Error;

    /// Build the operator `spec` names.
    ///
    /// Fails with [`Configuration`](elide::ErrorKind::Configuration)
    /// when elide rejects a parameter: a frequency that is not
    /// finite and positive. An amplitude outside `0.0..=1.0` is
    /// clamped.
    fn try_from(spec: &AudioRedaction) -> Result<Self> {
        Ok(match spec {
            AudioRedaction::Remove => Self::Remove,
            AudioRedaction::Keep => Self::Keep,
            AudioRedaction::Cover => Self::Cover,
            AudioRedaction::Beep {
                hz,
                amplitude,
                waveform,
            } => Self::Beep(
                Beep::new(*hz)?
                    .with_amplitude(*amplitude)
                    .with_waveform(*waveform),
            ),
        })
    }
}
