//! Image accessors for [`Document`].

use futures::StreamExt;
use nvisy_codec::Located;
use nvisy_codec::handler::{ImageData, ImageRedaction, Redactions};
use nvisy_core::Error;
use nvisy_ontology::entity::ImageLocation;

use super::Document;

impl Document {
    /// Collect all image locations into a `Vec`.
    pub async fn collect_image_locations(&self) -> Vec<Located<ImageLocation>> {
        self.handle.image_locations().collect().await
    }

    /// Read the image data at the given image location.
    pub async fn read_image(&self, location: &ImageLocation) -> Option<ImageData> {
        self.handle.read_image(location).await
    }

    /// Apply a batch of image redactions to the document.
    pub async fn apply_image_redactions(
        &mut self,
        redactions: Redactions<ImageLocation, ImageRedaction>,
    ) -> Result<(), Error> {
        self.handle.apply_image_redactions(redactions).await
    }
}
