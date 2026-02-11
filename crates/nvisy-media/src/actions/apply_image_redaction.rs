//! Image redaction action — applies blur or block overlay to image regions.

use bytes::Bytes;
use serde::Deserialize;
use tokio::sync::mpsc;

use nvisy_core::datatypes::blob::Blob;
use nvisy_core::datatypes::document::ImageData;
use nvisy_ontology::ontology::entity::{BoundingBox, Entity};
use nvisy_ontology::ontology::redaction::{Redaction, RedactionMethod};
use nvisy_core::error::{Error, ErrorKind};
use nvisy_core::registry::action::Action;

use crate::render::{blur, block};

/// Typed parameters for [`ApplyImageRedactionAction`].
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyImageRedactionParams {
    /// Sigma value for gaussian blur.
    #[serde(default = "default_sigma")]
    pub blur_sigma: f32,
    /// RGBA color for block overlays.
    #[serde(default = "default_color")]
    pub block_color: [u8; 4],
}

fn default_sigma() -> f32 {
    15.0
}
fn default_color() -> [u8; 4] {
    [0, 0, 0, 255]
}

/// Applies blur or block redaction to image regions identified by entities
/// with bounding boxes.
pub struct ApplyImageRedactionAction;

#[async_trait::async_trait]
impl Action for ApplyImageRedactionAction {
    type Params = ApplyImageRedactionParams;

    fn id(&self) -> &str {
        "apply-image-redaction"
    }

    fn validate_params(&self, _params: &Self::Params) -> Result<(), Error> {
        Ok(())
    }

    async fn execute(
        &self,
        mut input: mpsc::Receiver<Blob>,
        output: mpsc::Sender<Blob>,
        params: Self::Params,
    ) -> Result<u64, Error> {
        let mut count = 0u64;

        while let Some(mut blob) = input.recv().await {
            let images: Vec<ImageData> = blob.get_artifacts("images").map_err(|e| {
                Error::new(ErrorKind::Runtime, format!("failed to read images: {e}"))
            })?;
            let entities: Vec<Entity> = blob.get_artifacts("entities").map_err(|e| {
                Error::new(ErrorKind::Runtime, format!("failed to read entities: {e}"))
            })?;
            let redactions: Vec<Redaction> = blob.get_artifacts("redactions").map_err(|e| {
                Error::new(ErrorKind::Runtime, format!("failed to read redactions: {e}"))
            })?;

            // Build entity->redaction map
            let redaction_map: std::collections::HashMap<uuid::Uuid, &Redaction> = redactions
                .iter()
                .filter(|r| !r.applied)
                .map(|r| (r.entity_id, r))
                .collect();

            // Collect entities with bounding boxes, grouped by redaction method
            let mut blur_regions: Vec<BoundingBox> = Vec::new();
            let mut block_regions: Vec<BoundingBox> = Vec::new();

            for entity in &entities {
                if let Some(bbox) = &entity.location.bounding_box {
                    if let Some(redaction) = redaction_map.get(&entity.data.id) {
                        match redaction.method {
                            RedactionMethod::Blur => blur_regions.push(bbox.clone()),
                            RedactionMethod::Block => block_regions.push(bbox.clone()),
                            // Default non-image methods to block for images
                            _ => block_regions.push(bbox.clone()),
                        }
                    }
                }
            }

            if !blur_regions.is_empty() || !block_regions.is_empty() {
                // Process each image
                let mut new_images = Vec::new();
                for img in &images {
                    let dyn_img = image::load_from_memory(&img.image_data).map_err(|e| {
                        Error::new(ErrorKind::Runtime, format!("image decode failed: {e}"))
                    })?;

                    let mut result = dyn_img;
                    if !blur_regions.is_empty() {
                        result = blur::apply_gaussian_blur(&result, &blur_regions, params.blur_sigma);
                    }
                    if !block_regions.is_empty() {
                        let color = image::Rgba(params.block_color);
                        result = block::apply_block_overlay(&result, &block_regions, color);
                    }

                    // Encode back to PNG
                    let mut buf = std::io::Cursor::new(Vec::new());
                    result
                        .write_to(&mut buf, image::ImageFormat::Png)
                        .map_err(|e| {
                            Error::new(ErrorKind::Runtime, format!("image encode failed: {e}"))
                        })?;

                    let new_img = ImageData::new(
                        Bytes::from(buf.into_inner()),
                        "image/png",
                    )
                    .with_dimensions(result.width(), result.height());

                    new_images.push(new_img);
                    count += 1;
                }

                // Replace images artifact
                blob.artifacts.remove("images");
                for img in &new_images {
                    blob.add_artifact("images", img).map_err(|e| {
                        Error::new(ErrorKind::Runtime, format!("failed to add image: {e}"))
                    })?;
                }

                // Mark redactions as applied
                let mut updated_redactions: Vec<Redaction> = redactions.clone();
                for r in &mut updated_redactions {
                    if redaction_map.contains_key(&r.entity_id) {
                        r.applied = true;
                    }
                }
                blob.artifacts.remove("redactions");
                for r in &updated_redactions {
                    blob.add_artifact("redactions", r).map_err(|e| {
                        Error::new(ErrorKind::Runtime, format!("failed to add redaction: {e}"))
                    })?;
                }
            }

            if output.send(blob).await.is_err() {
                return Ok(count);
            }
        }

        Ok(count)
    }
}
