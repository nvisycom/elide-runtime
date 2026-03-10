//! WAV loader: wraps raw audio bytes into a [`WavHandler`].

use nvisy_core::Error;
use nvisy_core::io::ContentData;
use nvisy_core::path::ContentSource;

use crate::handler::{Loader, WavHandler};

/// Parameters for [`WavLoader`].
#[derive(Debug, Default)]
pub struct WavParams;

/// Loader that wraps raw WAV bytes.
///
/// Produces a single [`WavHandler`] per input.
#[derive(Debug, Default)]
pub struct WavLoader;

#[async_trait::async_trait]
impl Loader for WavLoader {
    type Handler = WavHandler;
    type Params = WavParams;

    #[tracing::instrument(name = "wav.decode", skip_all, fields(input_bytes))]
    async fn decode(
        &self,
        content: &ContentData,
        _params: &Self::Params,
    ) -> Result<WavHandler, Error> {
        tracing::Span::current().record("input_bytes", content.to_bytes().len());
        let source = ContentSource::new().with_parent(&content.content_source);
        let handler = WavHandler::new(content.to_bytes()).with_source(source);
        Ok(handler)
    }
}
