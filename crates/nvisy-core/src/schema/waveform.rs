//! [`WaveformSchema`]: wire shape for the audio
//! [`elide_core::modality::audio::Waveform`] tone shape, used by
//! [`AudioRedaction::Beep`].
//!
//! [`AudioRedaction::Beep`]: crate::policy::redaction::AudioRedaction::Beep

use elide_core::modality::audio::Waveform;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Tone shape for the [`AudioRedaction::Beep`] operator.
///
/// [`AudioRedaction::Beep`]: crate::policy::redaction::AudioRedaction::Beep
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "Waveform")]
pub enum WaveformSchema {
    /// Pure sine. The broadcast censor-beep convention.
    #[default]
    Sine,
    /// Square wave. Harsher and richer in harmonics — the "retro" bleep.
    Square,
}

impl From<WaveformSchema> for Waveform {
    fn from(s: WaveformSchema) -> Self {
        match s {
            WaveformSchema::Sine => Waveform::Sine,
            WaveformSchema::Square => Waveform::Square,
        }
    }
}

impl From<Waveform> for WaveformSchema {
    fn from(w: Waveform) -> Self {
        match w {
            Waveform::Sine => Self::Sine,
            Waveform::Square => Self::Square,
        }
    }
}
