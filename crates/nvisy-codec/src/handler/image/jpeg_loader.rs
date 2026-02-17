//! JPEG loader — validates and decodes raw JPEG bytes into a
//! [`Document<JpegHandler>`].

use image::DynamicImage;

use nvisy_core::error::{Error, ErrorKind};
use nvisy_core::io::ContentData;

use crate::document::Document;
use crate::handler::{Loader, JpegHandler};

/// Parameters for [`JpegLoader`].
#[derive(Debug, Default)]
pub struct JpegParams;

/// Loader that validates and decodes JPEG files.
///
/// Produces a single [`Document<JpegHandler>`] per input.
#[derive(Debug)]
pub struct JpegLoader;

#[async_trait::async_trait]
impl Loader for JpegLoader {
    type Handler = JpegHandler;
    type Params = JpegParams;

    async fn load(
        &self,
        content: &ContentData,
        _params: &Self::Params,
    ) -> Result<Vec<Document<JpegHandler>>, Error> {
        let raw = content.to_bytes();
        let image: DynamicImage = image::load_from_memory(&raw)
            .map_err(|e| Error::new(ErrorKind::Runtime, format!("JPEG decode failed: {e}")))?;

        let handler = JpegHandler::new(image);
        let doc = Document::new(handler).with_parent(content);
        Ok(vec![doc])
    }
}
