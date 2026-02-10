use async_trait::async_trait;

use crate::datatypes::blob::Blob;
use crate::datatypes::document::Document;
use crate::datatypes::image::ImageData;
use crate::errors::NvisyError;

/// Output of a loader: either a Document or an ImageData.
pub enum LoaderOutput {
    Document(Document),
    Image(ImageData),
}

/// A loader transforms Blobs into Documents or Images.
#[async_trait]
pub trait Loader: Send + Sync + 'static {
    fn id(&self) -> &str;
    fn extensions(&self) -> &[&str];
    fn content_types(&self) -> &[&str];

    async fn load(
        &self,
        blob: &Blob,
        params: &serde_json::Value,
    ) -> Result<Vec<LoaderOutput>, NvisyError>;
}
