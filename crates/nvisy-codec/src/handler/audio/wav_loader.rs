//! WAV loader: wraps raw audio bytes into a [`Document<WavHandler>`].

use nvisy_core::Error;
use nvisy_core::io::ContentData;

use crate::document::Document;
use crate::handler::{Loader, WavHandler};

/// Parameters for [`WavLoader`].
#[derive(Debug, Default)]
pub struct WavParams;

/// Loader that wraps raw WAV bytes.
///
/// Produces a single [`Document<WavHandler>`] per input.
#[derive(Debug)]
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
    ) -> Result<Vec<Document<WavHandler>>, Error> {
        tracing::Span::current().record("input_bytes", content.to_bytes().len());
        let handler = WavHandler::new(content.to_bytes());
        let doc = Document::new(handler).with_parent(content);
        Ok(vec![doc])
    }
}
