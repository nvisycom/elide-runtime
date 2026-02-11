//! Image file loader using the `image` crate.

use bytes::Bytes;
use serde::Deserialize;

use nvisy_core::datatypes::blob::Blob;
use nvisy_core::datatypes::document::ImageData;
use nvisy_core::error::{Error, ErrorKind};
use super::{Loader, LoaderOutput};

/// Typed parameters for [`ImageLoader`].
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageLoaderParams {}

/// Decodes image files and returns an [`ImageData`] with dimensions.
pub struct ImageLoader;

#[async_trait::async_trait]
impl Loader for ImageLoader {
    type Params = ImageLoaderParams;

    fn id(&self) -> &str {
        "image"
    }

    fn extensions(&self) -> &[&str] {
        &["jpg", "jpeg", "png", "tiff", "bmp", "webp"]
    }

    fn content_types(&self) -> &[&str] {
        &[
            "image/jpeg",
            "image/png",
            "image/tiff",
            "image/bmp",
            "image/webp",
        ]
    }

    async fn load(
        &self,
        blob: &Blob,
        _params: &Self::Params,
    ) -> Result<Vec<LoaderOutput>, Error> {
        let img = image::load_from_memory(&blob.content).map_err(|e| {
            Error::new(ErrorKind::Runtime, format!("Image decode failed: {e}"))
        })?;

        let width = img.width();
        let height = img.height();

        // Detect MIME type from blob or infer
        let mime_type = blob
            .content_type()
            .unwrap_or("image/png")
            .to_string();

        let image_data = ImageData::new(
            Bytes::copy_from_slice(&blob.content),
            mime_type,
        )
        .with_dimensions(width, height);

        Ok(vec![LoaderOutput::Image(image_data)])
    }
}
