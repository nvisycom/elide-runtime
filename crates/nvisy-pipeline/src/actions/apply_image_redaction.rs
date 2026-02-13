//! Image redaction action -- applies blur or block overlay to image regions.

use bytes::Bytes;
use serde::Deserialize;

use nvisy_ingest::handler::{FormatHandler, ImageHandler};
use nvisy_ingest::document::Document;
use nvisy_ontology::entity::{BoundingBox, Entity};
use nvisy_ontology::redaction::{ImageRedactionOutput, Redaction, RedactionOutput};
use nvisy_core::error::{Error, ErrorKind};

use crate::action::Action;
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
pub struct ApplyImageRedactionAction {
    params: ApplyImageRedactionParams,
}

#[async_trait::async_trait]
impl Action for ApplyImageRedactionAction {
    type Params = ApplyImageRedactionParams;
    type Input = (Vec<Document<FormatHandler>>, Vec<Entity>, Vec<Redaction>);
    type Output = Vec<Document<FormatHandler>>;

    fn id(&self) -> &str {
        "apply-image-redaction"
    }

    async fn connect(params: Self::Params) -> Result<Self, Error> {
        Ok(Self { params })
    }

    async fn execute(
        &self,
        input: Self::Input,
    ) -> Result<Self::Output, Error> {
        let (documents, entities, redactions) = input;

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
            if let Some(bbox) = entity.location.bounding_box() {
                if let Some(redaction) = redaction_map.get(&entity.source.as_uuid()) {
                    match &redaction.output {
                        RedactionOutput::Image(ImageRedactionOutput::Blur { .. }) => {
                            blur_regions.push(bbox.clone())
                        }
                        RedactionOutput::Image(ImageRedactionOutput::Block { .. }) => {
                            block_regions.push(bbox.clone())
                        }
                        // Default non-image methods, pixelate, and synthesize to block
                        _ => block_regions.push(bbox.clone()),
                    }
                }
            }
        }

        if blur_regions.is_empty() && block_regions.is_empty() {
            return Ok(documents);
        }

        // Filter for image documents only
        let mut new_docs = Vec::new();
        for doc in &documents {
            let image_data = match &doc.data {
                Some(d) => d,
                None => {
                    new_docs.push(doc.clone());
                    continue;
                }
            };

            let dyn_img = image::load_from_memory(image_data).map_err(|e| {
                Error::new(ErrorKind::Runtime, format!("image decode failed: {e}"))
            })?;

            let mut result = dyn_img;
            if !blur_regions.is_empty() {
                result = blur::apply_gaussian_blur(&result, &blur_regions, self.params.blur_sigma);
            }
            if !block_regions.is_empty() {
                let color = image::Rgba(self.params.block_color);
                result = block::apply_block_overlay(&result, &block_regions, color);
            }

            // Encode back to PNG
            let mut buf = std::io::Cursor::new(Vec::new());
            result
                .write_to(&mut buf, image::ImageFormat::Png)
                .map_err(|e| {
                    Error::new(ErrorKind::Runtime, format!("image encode failed: {e}"))
                })?;

            let new_doc = Document::new(FormatHandler::Image(ImageHandler))
                .with_data(Bytes::from(buf.into_inner()), "image/png")
                .with_dimensions(result.width(), result.height());

            new_docs.push(new_doc);
        }

        Ok(new_docs)
    }
}
