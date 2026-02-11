//! Plain-text file loader.

use nvisy_core::datatypes::blob::Blob;
use nvisy_core::datatypes::document::Document;
use nvisy_core::error::Error;
use super::{Loader, LoaderOutput};

/// Loads plain-text blobs into a single [`Document`].
///
/// The loader validates that the blob content is valid UTF-8 and tags the
/// resulting document with `source_format = "txt"`. It handles the `text/plain`
/// content type and `.txt` / `.text` file extensions.
pub struct PlaintextLoader;

#[async_trait::async_trait]
impl Loader for PlaintextLoader {
    type Params = ();

    fn id(&self) -> &str {
        "plaintext"
    }

    fn extensions(&self) -> &[&str] {
        &["txt", "text"]
    }

    fn content_types(&self) -> &[&str] {
        &["text/plain"]
    }

    async fn load(
        &self,
        blob: &Blob,
        _params: &Self::Params,
    ) -> Result<Vec<LoaderOutput>, Error> {
        let content = String::from_utf8(blob.content.to_vec()).map_err(|e| {
            Error::validation(
                format!("Invalid UTF-8 in plaintext: {}", e),
                "plaintext-loader",
            )
        })?;
        let mut doc = Document::new(content);
        doc.source_format = Some("txt".to_string());
        doc.data.parent_id = Some(blob.data.id);
        Ok(vec![LoaderOutput::Document(doc)])
    }
}
