//! Image file loader using the `image` crate.

use bytes::Bytes;
use serde::Deserialize;

use nvisy_core::io::ContentData;
use nvisy_core::error::{Error, ErrorKind};

use crate::document::Document;
use crate::handler::{ImageHandler as ImageHandlerType, FormatHandler, ImageLoader};

/// Typed parameters for [`ImageFileLoader`].
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageLoaderParams {}

/// Decodes image files and returns a [`Document`] with binary data and dimensions.
pub struct ImageFileLoader;

impl Clone for ImageFileLoader {
    fn clone(&self) -> Self { Self }
}

#[async_trait::async_trait]
impl ImageLoader for ImageFileLoader {
    type Params = ImageLoaderParams;

    async fn load(
        &self,
        content: &ContentData,
        _params: &Self::Params,
    ) -> Result<Vec<Document<FormatHandler>>, Error> {
        let raw = content.to_bytes();
        let img = image::load_from_memory(&raw).map_err(|e| {
            Error::new(ErrorKind::Runtime, format!("Image decode failed: {e}"))
        })?;

        let width = img.width();
        let height = img.height();

        let mime_type = content
            .content_type()
            .unwrap_or("image/png")
            .to_string();

        let mut doc = Document::new(ImageHandlerType)
            .with_data(Bytes::copy_from_slice(&raw), mime_type)
            .with_dimensions(width, height);
        doc.source.set_parent_id(Some(content.content_source.as_uuid()));
        Ok(vec![doc.into_format()])
    }
}

impl crate::handler::Handler for ImageFileLoader {
    fn id(&self) -> &str { ImageHandlerType.id() }
    fn extensions(&self) -> &[&str] { ImageHandlerType.extensions() }
    fn content_types(&self) -> &[&str] { ImageHandlerType.content_types() }
}
