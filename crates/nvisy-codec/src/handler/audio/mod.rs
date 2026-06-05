//! Audio-modality wire types: [`Codable`] impl.
//!
//! The per-modality capability surface lives on the generic
//! [`Handle<Audio>`] trait in [`crate::core`]. Concrete per-format
//! implementations (WAV, MP3) live in `nvisy-formats`. Audio handlers
//! call [`sort_redactions_for_audio`] inside their
//! [`IndexedHandle::redact`] impl so spans are applied right-to-left
//! (an [`AudioReplacement::Remove`] shrinks the buffer and shifts
//! every later sample index; right-to-left order keeps earlier
//! indices valid).
//!
//! Replacements written during [`IndexedHandle::redact`] use
//! [`nvisy_core::redaction::AudioReplacement`].
//!
//! [`Handle<Audio>`]: crate::core::Handle
//! [`IndexedHandle::redact`]: crate::core::IndexedHandle::redact
//! [`AudioReplacement::Remove`]: nvisy_core::redaction::AudioReplacement::Remove

use std::cmp::Reverse;

use nvisy_core::extraction::Redactions;
use nvisy_core::modality::{Audio, AudioLocation, ModalityKind};
use nvisy_core::redaction::AudioReplacement;

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
