//! Translation from a Textract response into elide's image vocabulary.
//!
//! Textract reports geometry as **ratios of the page**, and carries no
//! page dimensions anywhere — `DocumentMetadata` holds only a page count.
//! The caller supplies them on [`OcrRequest::dimensions`], which is what
//! [`UnitBoundingBox::denormalize`] converts against.
//!
//! Confidence arrives on a **0–100** scale, so it is divided before
//! reaching [`Confidence::clamped`], which clamps rather than rescales —
//! passing it through raw would saturate every value to 1.0.
//!
//! [`OcrRequest::dimensions`]: elide_image::ocr::OcrRequest::dimensions

use aws_sdk_textract::types::{Block, BlockType};
use elide_core::primitive::Confidence;
use elide_image::modality::{ImageLocation, LayoutRegion};
use elide_image::ocr::OcrResponse;
use elide_image::primitive::{Dimensions, UnitBoundingBox};

/// Textract's confidence scale.
const CONFIDENCE_SCALE: f32 = 100.0;

/// The transcript of a detected document.
///
/// Emits one region per `WORD` block — Textract's finest granularity, and
/// where a redaction pipeline wants its spans. `LINE` and `PAGE` blocks
/// are the same text at coarser groupings, so keeping them would emit
/// every span two or three times over.
pub(super) fn decode(blocks: &[Block], dimensions: Dimensions<u32>) -> OcrResponse {
    let regions = blocks
        .iter()
        .filter(|block| block.block_type == Some(BlockType::Word))
        .filter_map(|block| region(block, dimensions))
        .collect();
    OcrResponse::new(regions)
}

/// One region from a word block, or `None` when it has no usable geometry
/// or text.
fn region(block: &Block, dimensions: Dimensions<u32>) -> Option<LayoutRegion> {
    let text = block.text.as_deref()?.trim();
    if text.is_empty() {
        return None;
    }

    let bbox = block.geometry.as_ref()?.bounding_box.as_ref()?;
    // Ratios, so anything outside the unit square describes a box off the
    // page: the provider and the caller disagree about the image, and a
    // denormalized guess would redact the wrong pixels.
    let unit = unit_box(bbox.left, bbox.top, bbox.width, bbox.height)?;

    let location = ImageLocation {
        bounding_box: unit.denormalize(dimensions),
        polygon: None,
        // Textract reports `page` only for multi-page inputs (PDF, TIFF);
        // a single image carries none, and elide reads `None` as "the one
        // page" rather than page zero.
        page: block
            .page
            .and_then(|p| u32::try_from(p).ok())
            .filter(|p| *p > 0),
    };

    let mut layout = LayoutRegion::new(location, text.to_owned());
    if let Some(confidence) = block.confidence {
        layout = layout.with_confidence(Confidence::clamped(confidence / CONFIDENCE_SCALE));
    }
    Some(layout)
}

/// A unit box from Textract's origin-plus-size ratios, or `None` when they
/// are not a sane unit-square region.
fn unit_box(left: f32, top: f32, width: f32, height: f32) -> Option<UnitBoundingBox> {
    let values = [left, top, width, height];
    if values.iter().any(|v| !v.is_finite() || *v < 0.0) {
        return None;
    }
    if width <= 0.0
        || height <= 0.0
        || left + width > 1.0 + f32::EPSILON
        || top + height > 1.0 + f32::EPSILON
    {
        return None;
    }
    Some(UnitBoundingBox::new(
        f64::from(left),
        f64::from(top),
        f64::from(width),
        f64::from(height),
    ))
}

#[cfg(test)]
mod tests {
    use aws_sdk_textract::types::{BoundingBox as AwsBoundingBox, Geometry};

    use super::*;

    fn word(text: &str, confidence: f32, bbox: (f32, f32, f32, f32)) -> Block {
        let (left, top, width, height) = bbox;
        Block::builder()
            .block_type(BlockType::Word)
            .text(text)
            .confidence(confidence)
            .geometry(
                Geometry::builder()
                    .bounding_box(
                        AwsBoundingBox::builder()
                            .left(left)
                            .top(top)
                            .width(width)
                            .height(height)
                            .build(),
                    )
                    .build(),
            )
            .build()
    }

    fn dims() -> Dimensions<u32> {
        Dimensions::new(1000, 800)
    }

    /// Ratios become pixels against the caller's dimensions, and the 0–100
    /// confidence is rescaled rather than clamped.
    #[test]
    fn denormalizes_against_the_callers_dimensions() {
        let response = decode(&[word("hello", 95.0, (0.1, 0.25, 0.2, 0.5))], dims());
        let region = &response.regions[0];

        assert_eq!(region.text, "hello");
        // Ratios arrive as `f32`, so `0.1` is not exactly a tenth and the
        // widened product lands within a rounding step of the pixel.
        let box_ = &region.region.bounding_box;
        assert!(
            (box_.min.x - 100.0).abs() < 1e-3,
            "min.x was {}",
            box_.min.x
        );
        assert!(
            (box_.min.y - 200.0).abs() < 1e-3,
            "min.y was {}",
            box_.min.y
        );
        assert!(
            (box_.max.x - 300.0).abs() < 1e-3,
            "max.x was {}",
            box_.max.x
        );
        assert!(
            (box_.max.y - 600.0).abs() < 1e-3,
            "max.y was {}",
            box_.max.y
        );
    }

    /// The 0–100 scale must be divided: `Confidence::clamped` clamps, so a
    /// raw 95.0 would saturate to 1.0 and every word would look certain.
    #[test]
    fn rescales_confidence_from_the_aws_scale() {
        let response = decode(&[word("hello", 42.0, (0.0, 0.0, 0.5, 0.5))], dims());
        let confidence = response.regions[0].confidence.expect("a confidence");

        assert!((f32::from(confidence) - 0.42).abs() < 1e-6);
    }

    /// Only `WORD` blocks are emitted. Textract repeats the same text as
    /// `LINE` and `PAGE` groupings, so keeping them would emit every span
    /// two or three times over.
    #[test]
    fn keeps_only_word_blocks() {
        let mut line = word("hello world", 90.0, (0.0, 0.0, 0.9, 0.2));
        line.block_type = Some(BlockType::Line);
        let response = decode(&[line, word("hello", 95.0, (0.0, 0.0, 0.4, 0.2))], dims());

        assert_eq!(response.regions.len(), 1);
        assert_eq!(response.regions[0].text, "hello");
    }

    /// A box outside the unit square means the provider and the caller
    /// disagree about the image; a denormalized guess would redact the
    /// wrong pixels.
    #[test]
    fn drops_boxes_outside_the_unit_square() {
        assert!(unit_box(0.9, 0.0, 0.5, 0.2).is_none());
        assert!(unit_box(0.0, 0.0, -0.1, 0.2).is_none());
        assert!(unit_box(0.0, 0.0, 0.0, 0.2).is_none());
        assert!(unit_box(f32::NAN, 0.0, 0.5, 0.2).is_none());
        // A box filling the page exactly is fine.
        assert!(unit_box(0.0, 0.0, 1.0, 1.0).is_some());
    }

    /// A word with no text, or only whitespace, yields no region.
    #[test]
    fn drops_empty_words() {
        assert!(
            decode(&[word("   ", 95.0, (0.0, 0.0, 0.5, 0.5))], dims())
                .regions
                .is_empty()
        );
    }
}
