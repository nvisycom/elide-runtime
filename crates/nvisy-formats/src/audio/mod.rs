//! Audio-format implementations: WAV, MP3.

#[cfg(feature = "mp3")]
mod mp3_handler;
#[cfg(feature = "mp3")]
mod mp3_loader;
#[cfg(feature = "wav")]
mod wav_handler;
#[cfg(feature = "wav")]
mod wav_loader;

#[cfg(feature = "mp3")]
pub use self::mp3_handler::Mp3Handler;
#[cfg(feature = "mp3")]
pub use self::mp3_loader::{Mp3Loader, Mp3Params};
#[cfg(feature = "wav")]
pub use self::wav_handler::WavHandler;
#[cfg(feature = "wav")]
pub use self::wav_loader::{WavLoader, WavParams};
