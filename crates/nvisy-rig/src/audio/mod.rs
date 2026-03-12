//! Audio services: speech-to-text transcription and text-to-speech generation.

pub mod stt;
pub mod tts;

pub use self::stt::SttProvider;
pub use self::tts::TtsProvider;
