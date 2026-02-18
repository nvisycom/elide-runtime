//! PNG loader — validates and decodes raw PNG bytes into a
//! [`Document<PngHandler>`].

use nvisy_core::error::Error;
use nvisy_core::io::ContentData;

use crate::document::Document;
use crate::handler::{Loader, PngHandler};

/// Parameters for [`PngLoader`].
#[derive(Debug, Default)]
pub struct PngParams;

/// Loader that validates and decodes PNG files.
///
/// Produces a single [`Document<PngHandler>`] per input.
#[derive(Debug)]
pub struct PngLoader;

#[async_trait::async_trait]
impl Loader for PngLoader {
    type Handler = PngHandler;
    type Params = PngParams;

    async fn decode(
        &self,
        content: &ContentData,
        _params: &Self::Params,
    ) -> Result<Vec<Document<PngHandler>>, Error> {
        let image = super::decode_image(content, "png-loader")?;
        let handler = PngHandler::new(image);
        let doc = Document::new(handler).with_parent(content);
        Ok(vec![doc])
    }
}
