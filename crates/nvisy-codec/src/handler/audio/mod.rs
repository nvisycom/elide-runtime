//! Audio modality: concrete audio format implementations (WAV, MP3).
//!
//! Audio handlers override [`Handler::redact`] to call
//! [`Redactions::sort_descending`] so spans are applied right-to-left
//! (an [`AudioReplacement::Remove`] shrinks the buffer and shifts
//! every later sample index; right-to-left order keeps earlier
//! indices valid). Replacements use [`AudioReplacement`].
//!
//! [`AudioReplacement`]: nvisy_core::redaction::AudioReplacement
//! [`AudioReplacement::Remove`]: nvisy_core::redaction::AudioReplacement::Remove
//! [`Redactions::sort_descending`]: nvisy_core::redaction::Redactions::sort_descending
//! [`Handler::redact`]: crate::Handler::redact

#[cfg(any(feature = "wav", feature = "mp3"))]
pub(crate) mod duration;
#[cfg(any(feature = "wav", feature = "mp3"))]
pub(crate) mod redact;

#[cfg(feature = "mp3")]
mod mp3_codec;
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
