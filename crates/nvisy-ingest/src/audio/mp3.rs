//! MP3 audio file loader.
//!
//! Returns a document with metadata only -- audio redaction is not yet implemented.

use serde::Deserialize;

use nvisy_core::io::ContentData;
use nvisy_core::error::Error;

use crate::document::Document;
use crate::handler::{Mp3Handler, FormatHandler, AudioLoader};

/// Typed parameters for [`Mp3Loader`].
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Mp3LoaderParams {}

/// Placeholder loader for MP3 audio files. Returns a metadata-only document.
pub struct Mp3Loader;

impl Clone for Mp3Loader {
    fn clone(&self) -> Self { Self }
}

#[async_trait::async_trait]
impl AudioLoader for Mp3Loader {
    type Params = Mp3LoaderParams;

    async fn load(
        &self,
        content: &ContentData,
        _params: &Self::Params,
    ) -> Result<Vec<Document<FormatHandler>>, Error> {
        let content_type = content.content_type().unwrap_or("audio/mpeg").to_string();
        let size = content.to_bytes().len();

        let mut doc = Document::new(Mp3Handler)
            .with_text(format!(
                "[Audio file: type={}, size={} bytes. Audio redaction not yet implemented.]",
                content_type, size
            ));
        doc.source.set_parent_id(Some(content.content_source.as_uuid()));
        Ok(vec![doc.into_format()])
    }
}

impl crate::handler::Handler for Mp3Loader {
    fn id(&self) -> &str { Mp3Handler.id() }
    fn extensions(&self) -> &[&str] { Mp3Handler.extensions() }
    fn content_types(&self) -> &[&str] { Mp3Handler.content_types() }
}
