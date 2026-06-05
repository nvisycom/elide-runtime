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

use std::cmp::Reverse;

use nvisy_core::modality::{Audio, AudioLocation, ModalityKind};

use crate::core::{Codable, Redactions};

mod instruction;

pub use self::instruction::{AudioOutput, AudioRedaction};

impl Codable for Audio {
    type Instruction = AudioRedaction;

    const KIND: ModalityKind = ModalityKind::Audio;
}

/// Sort an audio redaction batch right-to-left by `time_span.start_us`.
///
/// Returned in the order audio handlers should apply individual
/// redactions: later spans first so a `Remove` doesn't invalidate
/// earlier sample indices.
pub fn sort_redactions_for_audio(
    redactions: Redactions<AudioLocation, AudioRedaction>,
) -> Vec<(AudioLocation, AudioRedaction)> {
    let mut items = redactions.into_items();
    items.sort_by_key(|(loc, _)| Reverse(loc.time_span.start_us));
    items
}
