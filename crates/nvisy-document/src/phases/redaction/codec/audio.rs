//! `AudioStrategy → AudioRedaction` conversion.

use nvisy_codec::handler::{AudioOutput, AudioRedaction};
use nvisy_core::Result;

use crate::policy::AudioStrategy;

/// Convert an [`AudioStrategy`] into a codec [`AudioRedaction`].
pub(crate) fn to_audio_redaction(strategy: &AudioStrategy) -> Result<AudioRedaction> {
    let output = match strategy {
        AudioStrategy::Silence => AudioOutput::Silence,
        AudioStrategy::Remove => AudioOutput::Remove,
    };
    Ok(AudioRedaction::new(output))
}
