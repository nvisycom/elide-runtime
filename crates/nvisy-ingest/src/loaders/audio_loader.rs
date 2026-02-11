//! Placeholder audio file loader.
//!
//! Returns a document with metadata only — audio redaction is not yet implemented.

use serde::Deserialize;

use nvisy_core::datatypes::blob::Blob;
use nvisy_core::datatypes::document::Document;
use nvisy_core::error::Error;
use super::{Loader, LoaderOutput};

/// Typed parameters for [`AudioLoader`].
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioLoaderParams {}

/// Placeholder loader for audio files.  Returns a metadata-only document.
pub struct AudioLoader;

#[async_trait::async_trait]
impl Loader for AudioLoader {
    type Params = AudioLoaderParams;

    fn id(&self) -> &str {
        "audio"
    }

    fn extensions(&self) -> &[&str] {
        &["mp3", "wav", "flac", "ogg", "m4a"]
    }

    fn content_types(&self) -> &[&str] {
        &[
            "audio/mpeg",
            "audio/wav",
            "audio/flac",
            "audio/ogg",
            "audio/mp4",
        ]
    }

    async fn load(
        &self,
        blob: &Blob,
        _params: &Self::Params,
    ) -> Result<Vec<LoaderOutput>, Error> {
        let content_type = blob.content_type().unwrap_or("audio/unknown").to_string();
        let size = blob.content.len();

        let doc = Document::new(format!(
            "[Audio file: type={}, size={} bytes. Audio redaction not yet implemented.]",
            content_type, size
        ))
        .with_source_format("audio");

        Ok(vec![LoaderOutput::Document(doc)])
    }
}
