//! MP3 loader — wraps raw audio bytes into a [`Document<Mp3Handler>`].

use nvisy_core::error::Error;
use nvisy_core::io::ContentData;

use crate::document::Document;
use crate::handler::{Loader, Mp3Handler};

/// Parameters for [`Mp3Loader`].
#[derive(Debug, Default)]
pub struct Mp3Params;

/// Loader that wraps raw MP3 bytes.
///
/// Produces a single [`Document<Mp3Handler>`] per input.
#[derive(Debug)]
pub struct Mp3Loader;

#[async_trait::async_trait]
impl Loader for Mp3Loader {
    type Handler = Mp3Handler;
    type Params = Mp3Params;

    async fn decode(
        &self,
        content: &ContentData,
        _params: &Self::Params,
    ) -> Result<Vec<Document<Mp3Handler>>, Error> {
        let handler = Mp3Handler::new(content.to_bytes());
        let doc = Document::new(handler).with_parent(content);
        Ok(vec![doc])
    }
}
