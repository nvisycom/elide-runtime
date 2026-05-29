//! Audio-modality wire types: [`Codable`] impl, [`AudioData`], and
//! the redaction shapes.
//!
//! The per-modality capability surface lives on the generic
//! [`Handle<Audio>`] trait in [`crate::core`]. Concrete per-format
//! implementations (WAV, MP3) live in `nvisy-formats`. Audio handlers
//! override [`Handle::redact`] to call [`sort_redactions_for_audio`]
//! so spans are applied right-to-left (an [`AudioOutput::Remove`]
//! shrinks the buffer and shifts every later sample index;
//! right-to-left order keeps earlier indices valid).
//!
//! [`Handle<Audio>`]: crate::core::Handle
//! [`Handle::redact`]: crate::core::Handle::redact
//! [`AudioOutput::Remove`]: AudioOutput::Remove

use std::cmp::Reverse;

use nvisy_ontology::modality::Audio;

use crate::core::{Codable, Redactions};

mod audio_data;
mod instruction;

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
    let mut items = redactions.items;
    items.sort_by_key(|pair| Reverse(pair.location.time_span.start_us));
    items
        .into_iter()
        .map(|pair| (pair.location, pair.redaction))
        .collect()
}
