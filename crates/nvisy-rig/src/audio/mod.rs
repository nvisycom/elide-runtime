//! Audio services: speech-to-text transcription and text-to-speech generation.

mod base;
pub mod transcribe;

pub use base::TranscribeProvider;

#[cfg(feature = "audio")]
pub use base::AudioGenProvider;

#[cfg(feature = "audio")]
#[cfg_attr(docsrs, doc(cfg(feature = "audio")))]
pub mod generate;
