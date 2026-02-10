//! CSV file loader.

use nvisy_core::datatypes::blob::Blob;
use nvisy_core::datatypes::document::Document;
use nvisy_core::error::Error;
use nvisy_core::registry::loader::{Loader, LoaderOutput};

/// Loads CSV blobs into a single [`Document`] containing the raw CSV text.
///
/// The loader validates that the blob content is valid UTF-8 and tags the
/// resulting document with `source_format = "csv"`. It handles the `text/csv`
/// content type and `.csv` file extension.
pub struct CsvLoader;

#[async_trait::async_trait]
impl Loader for CsvLoader {
    type Params = ();

    fn id(&self) -> &str {
        "csv"
    }

    fn extensions(&self) -> &[&str] {
        &["csv"]
    }

    fn content_types(&self) -> &[&str] {
        &["text/csv"]
    }

    async fn load(
        &self,
        blob: &Blob,
        _params: &Self::Params,
    ) -> Result<Vec<LoaderOutput>, Error> {
        let content = String::from_utf8(blob.content.to_vec()).map_err(|e| {
            Error::validation(format!("Invalid UTF-8 in CSV: {}", e), "csv-loader")
        })?;
        let mut doc = Document::new(content);
        doc.source_format = Some("csv".to_string());
        doc.data.parent_id = Some(blob.data.id);
        Ok(vec![LoaderOutput::Document(doc)])
    }
}
