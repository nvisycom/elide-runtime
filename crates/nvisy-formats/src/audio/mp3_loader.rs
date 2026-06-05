//! MP3 loader: wraps raw audio bytes into a [`Mp3Handler`].

use async_trait::async_trait;
use nvisy_codec::handler::Loader;
use nvisy_core::Error;
use nvisy_core::content::{ContentData, ContentSource};
use nvisy_core::modality::Audio;

use super::Mp3Handler;

/// Loader that wraps raw MP3 bytes. Produces one [`Mp3Handler`] per input.
#[derive(Debug, Default)]
pub struct Mp3Loader;

#[async_trait]
impl Loader<Audio> for Mp3Loader {
    type Handler = Mp3Handler;

    #[tracing::instrument(name = "mp3.decode", skip_all, fields(input_bytes))]
    async fn decode(&self, content: ContentData) -> Result<Mp3Handler, Error> {
        tracing::Span::current().record("input_bytes", content.to_bytes().len());
        let parent = content.content_source;
        let bytes = content.to_bytes();
        let source = ContentSource::new().with_parent(&parent);
        Ok(Mp3Handler::new(bytes).with_source(source))
    }
}
