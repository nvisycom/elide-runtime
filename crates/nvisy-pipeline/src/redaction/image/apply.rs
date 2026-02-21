//! Image document redaction.

use std::collections::HashMap;
use uuid::Uuid;

use nvisy_codec::handler::PngHandler;
use nvisy_codec::document::Document;
use nvisy_codec::transform::{ImageRedaction, ImageRedactionOutput, ImageHandler};
use crate::ontology::Entity;
use crate::redaction::record::Redaction;
use crate::redaction::spec::RedactionSpec;
use crate::redaction::image::spec::ImageRedactionSpec;
use nvisy_core::Error;

/// Convert a `RedactionSpec::Image` into a codec [`ImageRedactionOutput`].
pub(crate) fn image_output_from_spec(spec: &RedactionSpec) -> Option<ImageRedactionOutput> {
    match spec {
        RedactionSpec::Image(img) => Some(match img {
            ImageRedactionSpec::Blur { sigma } => ImageRedactionOutput::Blur { sigma: *sigma },
            ImageRedactionSpec::Block { color } => ImageRedactionOutput::Block { color: *color },
            ImageRedactionSpec::Pixelate { block_size } => {
                ImageRedactionOutput::Pixelate { block_size: *block_size }
            }
            ImageRedactionSpec::Synthesize => {
                ImageRedactionOutput::Block { color: [0, 0, 0, 255] }
            }
        }),
        _ => None,
    }
}

pub(crate) async fn apply_image_doc(
    doc: &Document<PngHandler>,
    entity_map: &HashMap<Uuid, &Entity>,
    redaction_map: &HashMap<Uuid, &Redaction>,
) -> Result<Document<PngHandler>, Error> {
    let mut redactions: Vec<ImageRedaction> = Vec::new();

    for (&entity_id, redaction) in redaction_map {
        let entity = match entity_map.get(&entity_id) {
            Some(e) => e,
            None => continue,
        };

        let img_loc = match &entity.image_location {
            Some(loc) => loc,
            None => continue,
        };

        let output = match image_output_from_spec(&redaction.spec) {
            Some(o) => o,
            None => continue,
        };

        redactions.push(ImageRedaction {
            bounding_box: img_loc.bounding_box.clone(),
            output,
        });
    }

    if redactions.is_empty() {
        return Ok(doc.clone());
    }

    let mut result = doc.clone();
    result.handler_mut().redact_spans(&redactions).await?;
    result.source.set_parent_id(Some(doc.source.as_uuid()));
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::redaction::text::spec::TextRedactionSpec;

    #[test]
    fn image_output_blur() {
        let spec = RedactionSpec::Image(ImageRedactionSpec::Blur { sigma: 5.0 });
        assert_eq!(
            image_output_from_spec(&spec),
            Some(ImageRedactionOutput::Blur { sigma: 5.0 })
        );
    }

    #[test]
    fn image_output_block() {
        let spec = RedactionSpec::Image(ImageRedactionSpec::Block {
            color: [255, 0, 0, 255],
        });
        assert_eq!(
            image_output_from_spec(&spec),
            Some(ImageRedactionOutput::Block {
                color: [255, 0, 0, 255]
            })
        );
    }

    #[test]
    fn image_output_pixelate() {
        let spec = RedactionSpec::Image(ImageRedactionSpec::Pixelate { block_size: 8 });
        assert_eq!(
            image_output_from_spec(&spec),
            Some(ImageRedactionOutput::Pixelate { block_size: 8 })
        );
    }

    #[test]
    fn image_output_synthesize_maps_to_black_block() {
        let spec = RedactionSpec::Image(ImageRedactionSpec::Synthesize);
        assert_eq!(
            image_output_from_spec(&spec),
            Some(ImageRedactionOutput::Block {
                color: [0, 0, 0, 255]
            })
        );
    }

    #[test]
    fn image_output_text_spec_returns_none() {
        let spec = RedactionSpec::Text(TextRedactionSpec::Remove);
        assert_eq!(image_output_from_spec(&spec), None);
    }

    #[test]
    fn image_output_audio_spec_returns_none() {
        let spec = RedactionSpec::Audio(crate::redaction::audio::spec::AudioRedactionSpec::Silence);
        assert_eq!(image_output_from_spec(&spec), None);
    }
}
