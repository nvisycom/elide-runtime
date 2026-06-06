//! Audio modality: `impl Codable for Audio`, the
//! [`sort_redactions_for_audio`] helper, plus concrete audio format
//! implementations (WAV, MP3).
//!
//! Audio handlers override [`IndexedHandle::redact`] to call
//! [`sort_redactions_for_audio`] so spans are applied right-to-left
//! (an [`AudioReplacement::Remove`] shrinks the buffer and shifts
//! every later sample index; right-to-left order keeps earlier
//! indices valid). Replacements use
//! [`nvisy_core::redaction::AudioReplacement`].
//!
//! [`IndexedHandle::redact`]: crate::core::IndexedHandle::redact
//! [`AudioReplacement::Remove`]: nvisy_core::redaction::AudioReplacement::Remove

use std::cmp::Reverse;

use nvisy_core::modality::{Audio, AudioLocation, ModalityKind};
use nvisy_core::redaction::{AudioReplacement, Redactions};

use crate::core::Codable;

impl Codable for Audio {
    const KIND: ModalityKind = ModalityKind::Audio;
}

/// Sort an audio redaction batch right-to-left by `time_span.start_us`.
///
/// Returned in the order audio handlers should apply individual
/// replacements: later spans first so a [`AudioReplacement::Remove`]
/// doesn't invalidate earlier sample indices.
pub fn sort_redactions_for_audio(
    redactions: Redactions<Audio>,
) -> Vec<(AudioLocation, AudioReplacement)> {
    let mut items = redactions.into_items();
    items.sort_by_key(|(loc, _)| Reverse(loc.time_span.start_us));
    items
}

#[cfg(feature = "wav")]
pub(crate) mod redact;

#[cfg(feature = "mp3")]
mod mp3_handler;
#[cfg(feature = "mp3")]
mod mp3_loader;
#[cfg(feature = "wav")]
mod wav_handler;
#[cfg(feature = "wav")]
mod wav_loader;

#[cfg(feature = "mp3")]
pub use self::mp3_handler::{Mp3Handler, format as mp3_format};
#[cfg(feature = "mp3")]
pub use self::mp3_loader::Mp3Loader;
#[cfg(feature = "wav")]
pub use self::wav_handler::{WavHandler, format as wav_format};
#[cfg(feature = "wav")]
pub use self::wav_loader::WavLoader;
