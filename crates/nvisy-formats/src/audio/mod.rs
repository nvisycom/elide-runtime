//! Audio-format implementations: WAV, MP3.

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
