//! Audio-modality extraction.
//!
//! Today's only audio extraction technique is STT ([`stt`]). Future
//! techniques (e.g. speaker diarization as its own pass) would live
//! as sibling sub-modules and stack inside the audio arm of
//! [`ExtractionPhase::apply`].
//!
//! [`ExtractionPhase::apply`]: super::ExtractionPhase::apply

#[cfg(feature = "audio")]
pub mod stt;

#[cfg(feature = "audio")]
pub use self::stt::{SttExtractor, SttExtractorConfig};
