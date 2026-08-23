//! Audio-modality operator builder.
//!
//! Symmetric with [`super::text`] and [`super::image`]: builds
//! an [`AudioOp`] from an `AudioRedaction` spec, dispatches it
//! onto a [`Target`] with the right concrete elide operator
//! type. Audio operators do not cross modalities.

use elide::modality::audio::Audio;
use elide::redaction::Anonymizer;
use elide::redaction::operators::{Beep, Erase, Keep, Silence};
use elide_governance::redaction::AudioRedaction;

use crate::redaction::compile::Target;

/// Discriminated builder result so [`Target::attach_with`] can
/// attach the right concrete operator type. Same reason as
/// [`super::text::TextOp`]: [`Anonymizer::with_label`] takes
/// `O: Operator<M> + 'static` by value.
///
/// [`Anonymizer::with_label`]: elide::redaction::Anonymizer::with_label
pub(in crate::redaction) enum AudioOp {
    Erase,
    Keep,
    Silence,
    Beep(Beep),
}

impl AudioOp {
    /// Attach `self` to `target`.
    pub(in crate::redaction) fn attach_to(self, target: Target<'_, Audio>) -> Anonymizer<Audio> {
        match self {
            AudioOp::Erase => target.attach_with(Erase),
            AudioOp::Keep => target.attach_with(Keep),
            AudioOp::Silence => target.attach_with(Silence),
            AudioOp::Beep(op) => target.attach_with(op),
        }
    }
}

impl From<&AudioRedaction> for AudioOp {
    fn from(spec: &AudioRedaction) -> Self {
        match spec {
            AudioRedaction::Erase => Self::Erase,
            AudioRedaction::Keep => Self::Keep,
            AudioRedaction::Silence => Self::Silence,
            AudioRedaction::Beep {
                hz,
                amplitude,
                waveform,
            } => Self::Beep(
                Beep::new(*hz)
                    .with_amplitude(*amplitude)
                    .with_waveform(*waveform),
            ),
        }
    }
}
