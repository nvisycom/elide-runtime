//! [`DocumentHandle<Image>`] implements [`DataAt`] and
//! [`RedactAt`] for the [`Image`] modality by delegating to the
//! underlying [`Handler<Image>`].
//!
//! Image does not implement [`TextAt`]: "text at this location" for
//! an image is OCR text, which the extraction phase (in
//! `nvisy-toolkit::extraction`) produces and the engine stamps onto
//! document blocks. The codec layer has no visibility into OCR.
//!
//! [`DocumentHandle<Image>`]: crate::document::DocumentHandle
//! [`Handler<Image>`]: crate::core::Handler
//! [`TextAt`]: nvisy_core::extraction::TextAt
//! [`DataAt`]: nvisy_core::extraction::DataAt
//! [`RedactAt`]: nvisy_core::redaction::RedactAt
//! [`Image`]: nvisy_core::modality::Image

use nvisy_core::Result;
use nvisy_core::extraction::DataAt;
use nvisy_core::modality::{Image, ImageData, ImageLocation};
use nvisy_core::redaction::{RedactAt, Redactions};

use crate::document::DocumentHandle;

#[async_trait::async_trait]
impl DataAt<Image> for DocumentHandle<Image> {
    async fn data_at(&self, location: &ImageLocation) -> Option<ImageData> {
        self.handler().read(location).await.ok().flatten()
    }
}

#[async_trait::async_trait]
impl RedactAt<Image> for DocumentHandle<Image> {
    async fn redact_at(&mut self, redactions: Redactions<Image>) -> Result<()> {
        self.handler_mut().redact(redactions).await
    }
}
