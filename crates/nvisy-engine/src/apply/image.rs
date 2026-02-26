//! Image document redaction.

use std::collections::HashMap;
use uuid::Uuid;

use nvisy_codec::handler::PngHandler;
use nvisy_codec::document::Document;
use nvisy_codec::transform::{ImageRedaction, ImageRedactionOutput, ImageHandler};
use nvisy_ontology::entity::Entity;
use nvisy_ontology::location::Location;
use nvisy_ontology::record::Redaction;
use nvisy_ontology::specification::{ImageRedactionInput, RedactionInput};
use nvisy_core::Error;

/// Convert a `RedactionInput::Image` into a codec [`ImageRedactionOutput`].
pub(crate) fn image_output_from_spec(spec: &RedactionInput) -> Option<ImageRedactionOutput> {
    match spec {
        RedactionInput::Image(img) => Some(match img {
            ImageRedactionInput::Blur { sigma } => ImageRedactionOutput::Blur { sigma: *sigma },
            ImageRedactionInput::Block { color } => ImageRedactionOutput::Block { color: *color },
            ImageRedactionInput::Pixelate { block_size } => {
                ImageRedactionOutput::Pixelate { block_size: *block_size }
            }
            ImageRedactionInput::Synthesize => {
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

        let img_loc = match &entity.location {
            Some(Location::Image(loc)) => loc,
            _ => continue,
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
    use nvisy_ontology::specification::TextRedactionInput;

    #[test]
    fn image_output_blur() {
        let spec = RedactionInput::Image(ImageRedactionInput::Blur { sigma: 5.0 });
        assert_eq!(
            image_output_from_spec(&spec),
            Some(ImageRedactionOutput::Blur { sigma: 5.0 })
        );
    }

    #[test]
    fn image_output_block() {
        let spec = RedactionInput::Image(ImageRedactionInput::Block {
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
        let spec = RedactionInput::Image(ImageRedactionInput::Pixelate { block_size: 8 });
        assert_eq!(
            image_output_from_spec(&spec),
            Some(ImageRedactionOutput::Pixelate { block_size: 8 })
        );
    }

    #[test]
    fn image_output_synthesize_maps_to_black_block() {
        let spec = RedactionInput::Image(ImageRedactionInput::Synthesize);
        assert_eq!(
            image_output_from_spec(&spec),
            Some(ImageRedactionOutput::Block {
                color: [0, 0, 0, 255]
            })
        );
    }

    #[test]
    fn image_output_text_spec_returns_none() {
        let spec = RedactionInput::Text(TextRedactionInput::Remove);
        assert_eq!(image_output_from_spec(&spec), None);
    }

    #[test]
    fn image_output_audio_spec_returns_none() {
        let spec = RedactionInput::Audio(nvisy_ontology::specification::AudioRedactionInput::Silence);
        assert_eq!(image_output_from_spec(&spec), None);
    }
}
