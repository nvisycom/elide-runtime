//! Audio format handlers and loaders.

use nvisy_core::Error;

use super::Handler;
use crate::document::SpanStream;

mod audio_data;
mod audio_handler;
mod mp3_handler;
mod mp3_loader;
mod wav_handler;
mod wav_loader;

pub use audio_data::AudioData;
pub use audio_handler::BoxedAudioHandler;
pub use mp3_handler::Mp3Handler;
pub use mp3_loader::{Mp3Loader, Mp3Params};
pub use wav_handler::WavHandler;
pub use wav_loader::{WavLoader, WavParams};

/// Identifier for an audio span within a single-track handler.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AudioSpanId;

/// Capability trait for handlers that expose audio content.
///
/// Handlers implementing this trait can yield audio spans and accept
/// audio edits.
#[async_trait::async_trait]
pub trait AudioHandler: Handler {
    /// Strongly-typed identifier for an audio span within this handler.
    type AudioId: Send + Sync + Clone + 'static;

    /// Return audio content as an async stream of spans.
    async fn audio_spans(&self) -> SpanStream<'_, Self::AudioId, AudioData>;

    /// Apply audio edits from an async stream back to the source structure.
    async fn edit_audio(
        &mut self,
        edits: SpanStream<'_, Self::AudioId, AudioData>,
    ) -> Result<(), Error>;
}
