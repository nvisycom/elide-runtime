//! WAV audio file loader.
//!
//! Returns a document with metadata only -- audio redaction is not yet implemented.

use serde::Deserialize;

use nvisy_core::io::ContentData;
use nvisy_core::error::Error;

use crate::document::Document;
use crate::handler::{WavHandler, FormatHandler, AudioLoader};

/// Typed parameters for [`WavLoader`].
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WavLoaderParams {}

/// Placeholder loader for WAV audio files. Returns a metadata-only document.
pub struct WavLoader;

impl Clone for WavLoader {
    fn clone(&self) -> Self { Self }
}

#[async_trait::async_trait]
impl AudioLoader for WavLoader {
    type Params = WavLoaderParams;

    async fn load(
        &self,
        content: &ContentData,
        _params: &Self::Params,
    ) -> Result<Vec<Document<FormatHandler>>, Error> {
        let content_type = content.content_type().unwrap_or("audio/wav").to_string();
        let size = content.to_bytes().len();

        let mut doc = Document::new(WavHandler)
            .with_text(format!(
                "[Audio file: type={}, size={} bytes. Audio redaction not yet implemented.]",
                content_type, size
            ));
        doc.source.set_parent_id(Some(content.content_source.as_uuid()));
        Ok(vec![doc.into_format()])
    }
}

impl crate::handler::Handler for WavLoader {
    fn id(&self) -> &str { WavHandler.id() }
    fn extensions(&self) -> &[&str] { WavHandler.extensions() }
    fn content_types(&self) -> &[&str] { WavHandler.content_types() }
}
