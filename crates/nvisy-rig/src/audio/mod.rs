//! Audio services: speech-to-text transcription and text-to-speech generation.

pub mod stt;
pub mod tts;

pub use stt::SttProvider;
pub use tts::TtsProvider;
