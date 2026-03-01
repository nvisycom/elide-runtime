//! OCR result parsing.

use nvisy_ontology::entity::{DetectionMethod, Entity, EntityCategory, EntityKind};
use nvisy_ontology::location::{ImageLocation, Location};

use crate::backend::OcrRegion;

/// Convert typed [`OcrRegion`] results into [`Entity`] values.
pub fn parse_ocr_entities(regions: &[OcrRegion]) -> Vec<Entity> {
    regions
        .iter()
        .map(|r| {
            Entity::new(
                EntityCategory::Pii,
                EntityKind::Handwriting,
                &r.text,
                DetectionMethod::Ocr,
                r.confidence,
            )
            .with_location(Location::Image(ImageLocation {
                bounding_box: r.bbox.clone(),
                image_id: None,
                page_number: None,
            }))
        })
        .collect()
}
