//! The `Loader` trait for converting raw blobs into structured documents or images.

use serde::de::DeserializeOwned;

use crate::datatypes::blob::Blob;
use crate::datatypes::document::Document;
use crate::datatypes::document::ImageData;
use crate::error::Error;

/// Output of a loader -- either a parsed document or an extracted image.
pub enum LoaderOutput {
    /// A successfully parsed text document.
    Document(Document),
    /// An extracted or decoded image.
    Image(ImageData),
}

/// Converts raw [`Blob`] content into structured [`Document`]s or [`ImageData`].
///
/// Loaders declare which file extensions and MIME types they support.
/// The engine selects the appropriate loader based on the blob's
/// content type and extension.
#[async_trait::async_trait]
pub trait Loader: Send + Sync + 'static {
    /// Strongly-typed parameters for this loader.
    type Params: DeserializeOwned + Send;

    /// Unique identifier for this loader (e.g. `"csv"`, `"pdf"`).
    fn id(&self) -> &str;
    /// File extensions this loader handles (e.g. `["csv", "tsv"]`).
    fn extensions(&self) -> &[&str];
    /// MIME types this loader handles (e.g. `["text/csv"]`).
    fn content_types(&self) -> &[&str];

    /// Parse the blob and return one or more documents or images.
    async fn load(
        &self,
        blob: &Blob,
        params: &Self::Params,
    ) -> Result<Vec<LoaderOutput>, Error>;
}
