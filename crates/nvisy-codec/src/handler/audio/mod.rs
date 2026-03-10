//! Audio format handlers and loaders.

use nvisy_core::Error;

use super::Handler;
use crate::document::SpanStream;

mod audio_data;
mod audio_handler;
mod audio_handler_macro;
mod mp3_handler;
mod mp3_loader;
mod wav_handler;
mod wav_loader;

use audio_handler_macro::impl_audio_handler;

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
/// All audio handlers use [`AudioSpanId`] as their span identifier,
/// making this trait directly object-safe without a `Dyn*` wrapper.
#[async_trait::async_trait]
pub trait AudioHandler: Handler {
    /// Return audio content as an async stream of [`Span`](crate::document::Span)s.
    ///
    /// Each span carries an [`AudioSpanId`] and [`AudioData`] payload.
    async fn audio_spans(&self) -> SpanStream<'_, AudioSpanId, AudioData>;

    /// Apply audio edits from an async stream back to the handler.
    ///
    /// The stream items must use the same [`AudioSpanId`] returned by
    /// [`audio_spans`](Self::audio_spans).
    async fn edit_audio(
        &mut self,
        edits: SpanStream<'_, AudioSpanId, AudioData>,
    ) -> Result<(), Error>;
}
