//! WAV loader: wraps raw audio bytes into a [`WavHandler`].

use nvisy_core::Error;
use nvisy_core::modality::Audio;

use super::WavHandler;
use crate::Loader;
use crate::content::{ContentData, ContentSource};

/// Loader that wraps raw WAV bytes. Produces one [`WavHandler`] per input.
#[derive(Debug, Default)]
pub struct WavLoader;

#[async_trait::async_trait]
impl Loader<Audio> for WavLoader {
    type Handler = WavHandler;

    #[tracing::instrument(name = "wav.decode", skip_all, fields(input_bytes))]
    async fn decode(&self, content: ContentData) -> Result<WavHandler, Error> {
        tracing::Span::current().record("input_bytes", content.to_bytes().len());
        let parent = content.content_source;
        let bytes = content.to_bytes();
        let source = ContentSource::new().with_parent(&parent);
        Ok(WavHandler::new(bytes).with_source(source))
    }
}
