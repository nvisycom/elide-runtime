//! Audio-modality codec types: [`Codable`] impl, redaction shapes,
//! and the `apply_audio_redaction` helper.
//!
//! The per-modality capability surface lives on the generic
//! [`Handle<Audio>`] trait in [`super::handle`]. Concrete per-format
//! implementations (WAV, MP3) live in `nvisy-formats`. Audio handlers
//! override [`Handle::redact`] to call [`sort_redactions_for_audio`]
//! so spans are applied right-to-left (an
//! [`AudioOutput::Remove`] shrinks the buffer and shifts every later
//! sample index; right-to-left order keeps earlier indices valid).
//!
//! [`Handle<Audio>`]: super::Handle
//! [`Handle::redact`]: super::Handle::redact
//! [`AudioOutput::Remove`]: AudioOutput::Remove

use std::cmp::Reverse;

use nvisy_ontology::modality::Audio;

use super::{Codable, Redactions};

mod apply;
mod audio_data;
mod instruction;

pub use self::apply::apply_audio_redaction;
pub use self::audio_data::AudioData;
pub use self::instruction::{AudioOutput, AudioRedaction};

impl Codable for Audio {
    type Data = AudioData;
    type Redaction = AudioRedaction;
}

/// Sort an audio redaction batch right-to-left by `time_span.start_us`.
///
/// Returned in the order audio handlers should apply individual
/// redactions: later spans first so a `Remove` doesn't invalidate
/// earlier sample indices.
pub fn sort_redactions_for_audio(
    redactions: Redactions<Audio, AudioRedaction>,
) -> Vec<(Audio, AudioRedaction)> {
    let mut items: Vec<_> = redactions.into_iter().collect();
    items.sort_by_key(|(loc, _)| Reverse(loc.time_span.start_us));
    items
}
