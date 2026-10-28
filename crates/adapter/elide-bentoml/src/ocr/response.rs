//! Incoming wire types for the OCR `/recognize` endpoint.
//!
//! The wire types are generated from `elide_core.ocr.v1.OcrResponse`. The wire tree is `Page -> Block -> Line -> Word`;
//! elide's [`Layout`] is a flat list of regions, so
//! [`WireOcrResponse::decode`] keeps the words and discards the
//! groupings above them. The response-level `model_id`, per-page
//! `width`/`height`, per-block `kind`, and any rotated polygons are
//! deserialised-and-discarded for now.
//!
//! [`Layout`]: elide_image::modality::Layout

use elide_core::primitive::Confidence;
use elide_image::modality::{ImageLocation, LayoutRegion};
use elide_image::ocr::OcrResponse;
use elide_image::primitive::{BoundingBox, Point};

use super::wire::{BoundingBox as WireBoundingBox, OcrResponse as WireOcrResponse};
use crate::error::BentoError;

impl From<WireBoundingBox> for BoundingBox<f64> {
    fn from(b: WireBoundingBox) -> Self {
        BoundingBox::new(
            Point::new(b.x, b.y),
            Point::new(b.x + b.width, b.y + b.height),
        )
    }
}

impl WireOcrResponse {
    /// Translate into the elide [`OcrResponse`] the backend trait expects.
    ///
    /// [`Layout`] is a flat list of regions, so the wire's
    /// pages → blocks → lines → words nesting collapses into one.
    ///
    /// Emits at the deepest level that has content: words where they
    /// exist, since they are the finest granularity the service reports
    /// and the only level carrying a confidence, which a redaction
    /// pipeline wants per span rather than per paragraph. A block or line
    /// whose children are empty is emitted itself — the contract defaults
    /// both `lines` and `words` to empty while requiring `text`, so a
    /// conforming service can report text with no words, and discarding
    /// it would lose recognised content.
    ///
    /// [`Layout`]: elide_image::modality::Layout
    pub(super) fn decode(self) -> Result<OcrResponse, BentoError> {
        let mut regions = Vec::new();
        for page in self.pages {
            let page_number = Some(u32::try_from(page.page_number.get()).map_err(|_| {
                BentoError::Protocol("OCR page number exceeds the Rust domain range".into())
            })?);
            for block in page.blocks {
                // Descend as far as the response actually goes, and emit at
                // the deepest level that has content. A block or line may
                // carry text with no children — the contract defaults both
                // `lines` and `words` to empty — and dropping it would lose
                // recognised text a redaction pipeline has to see.
                if block.lines.is_empty() {
                    regions.push(region(block.bbox, block.text, None, page_number));
                    continue;
                }
                for line in block.lines {
                    if line.words.is_empty() {
                        regions.push(region(line.bbox, line.text, None, page_number));
                        continue;
                    }
                    for word in line.words {
                        regions.push(region(word.bbox, word.text, word.confidence, page_number));
                    }
                }
            }
        }
        Ok(OcrResponse::new(regions))
    }
}

/// One region from a wire bbox, its text, and an optional confidence.
fn region(
    bbox: WireBoundingBox,
    text: String,
    confidence: Option<f64>,
    page_number: Option<u32>,
) -> LayoutRegion {
    let location = ImageLocation {
        bounding_box: bbox.into(),
        polygon: None,
        page: page_number,
    };
    let layout = LayoutRegion::new(location, text);
    match confidence {
        Some(c) => layout.with_confidence(Confidence::clamped(c as f32)),
        None => layout,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ocr::wire::{BlockKind, OcrOutput};

    #[test]
    fn python_fixtures_preserve_hierarchy_geometry_and_fallbacks() {
        let cases: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../../contracts/ocr/v1/fixtures.json"
        ))
        .unwrap();
        for case in cases.as_array().unwrap() {
            let mut wire: OcrOutput = serde_json::from_value(case["response"].clone()).unwrap();
            if case["name"] == "word-polygon" {
                let block = &wire[0].pages[0].blocks[0];
                assert!(matches!(block.kind, BlockKind::Table));
                assert_eq!(block.lines[0].words[0].polygon.unwrap().2, (40.0, 24.0));
            }
            let regions = wire.0.remove(0).decode().unwrap().regions;
            let expected = case["expected_regions"].as_array().unwrap();
            assert_eq!(regions.len(), expected.len());
            for (region, expected) in regions.iter().zip(expected) {
                assert_eq!(region.text, expected["text"].as_str().unwrap());
                assert_eq!(
                    region.region.page.unwrap(),
                    expected["page"].as_u64().unwrap() as u32
                );
                let bbox = region.region.bounding_box;
                let expected_box = expected["bbox"].as_array().unwrap();
                assert_eq!(
                    [bbox.min.x, bbox.min.y, bbox.width(), bbox.height()],
                    std::array::from_fn(|i| expected_box[i].as_f64().unwrap())
                );
                assert_eq!(
                    region.confidence,
                    expected["confidence"]
                        .as_f64()
                        .map(|c| Confidence::clamped(c as f32))
                );
            }
        }
    }

    #[test]
    fn defaults_and_optional_metadata_match_python() {
        let response: WireOcrResponse = serde_json::from_str(
            r#"{
            "model_id":"m", "pages":[{"page_number":1,"blocks":[{
                "text":"block", "bbox":{"x":0,"y":0,"width":10,"height":4}
            }]}]}"#,
        )
        .unwrap();
        assert!(matches!(response.pages[0].blocks[0].kind, BlockKind::Text));
        assert_eq!(response.decode().unwrap().regions.len(), 1);
        let empty: WireOcrResponse = serde_json::from_str(r#"{"model_id":"m"}"#).unwrap();
        assert!(empty.decode().unwrap().regions.is_empty());
    }

    #[test]
    fn page_number_must_fit_the_domain() {
        let zero = serde_json::json!({"model_id":"m", "pages":[{"page_number":0}]});
        assert!(serde_json::from_value::<WireOcrResponse>(zero).is_err());
        let response: WireOcrResponse = serde_json::from_value(serde_json::json!({
            "model_id":"m", "pages":[{"page_number":u64::from(u32::MAX) + 1}]
        }))
        .unwrap();
        assert!(response.decode().is_err());
    }

    #[test]
    fn polygon_shape_is_enforced_by_generated_types() {
        let json = r#"{"text":"a","bbox":{"x":0,"y":0,"width":1,"height":1},"polygon":[[0,0],[1,0],[1,1]]}"#;
        assert!(serde_json::from_str::<crate::ocr::wire::Word>(json).is_err());
    }
}
