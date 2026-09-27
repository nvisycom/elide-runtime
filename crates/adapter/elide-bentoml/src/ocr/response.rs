//! Incoming wire types for the OCR `/recognize` endpoint.
//!
//! Mirrors `bento_core.ocr.v1.OcrResponse` from the inference
//! repository. The wire tree is `Page -> Block -> Line -> Word`;
//! elide's [`Layout`] is a flat list of regions, so
//! [`WireOcrResponse::decode`] keeps the words and discards the
//! groupings above them. The response-level `modelId`, per-page
//! `width`/`height`, per-block `kind`, and any rotated polygons are
//! deserialised-and-discarded for now.
//!
//! [`Layout`]: elide_image::modality::Layout

use elide_core::primitive::Confidence;
use elide_image::modality::{ImageLocation, LayoutRegion};
use elide_image::ocr::OcrResponse;
use elide_image::primitive::{BoundingBox, Point};
use serde::Deserialize;

/// Incoming per-call response body element.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct WireOcrResponse {
    #[serde(default)]
    pub pages: Vec<WirePage>,
    // `modelId` ignored: provenance comes from `BentoOcr::model_id`
    // (the deployment-level id the operator wired at construction).
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct WirePage {
    /// 1-based page index; flows straight onto [`ImageLocation::page`].
    pub page_number: Option<u32>,
    #[serde(default)]
    pub blocks: Vec<WireBlock>,
    // `width`, `height` ignored: elide's layout does not carry page
    // dimensions today.
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct WireBlock {
    pub text: String,
    pub bbox: WireBoundingBox,
    #[serde(default)]
    pub lines: Vec<WireLine>,
    // `kind` (text / table / figure / other) ignored: `LayoutRegion` does
    // not model a layout kind.
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct WireLine {
    pub text: String,
    pub bbox: WireBoundingBox,
    #[serde(default)]
    pub words: Vec<WireWord>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct WireWord {
    pub text: String,
    pub confidence: Option<f32>,
    pub bbox: WireBoundingBox,
    // `polygon` ignored: `ImageLocation` has a `polygon` field, but the
    // service reports one only for rotated regions, and nothing here
    // requests that mode yet.
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct WireBoundingBox {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

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
    pub(super) fn decode(self) -> OcrResponse {
        let mut regions = Vec::new();
        for page in self.pages {
            let page_number = page.page_number;
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
        OcrResponse::new(regions)
    }
}

/// One region from a wire bbox, its text, and an optional confidence.
fn region(
    bbox: WireBoundingBox,
    text: String,
    confidence: Option<f32>,
    page_number: Option<u32>,
) -> LayoutRegion {
    let location = ImageLocation {
        bounding_box: bbox.into(),
        polygon: None,
        page: page_number,
    };
    let layout = LayoutRegion::new(location, text);
    match confidence {
        Some(c) => layout.with_confidence(Confidence::clamped(c)),
        None => layout,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bbox() -> WireBoundingBox {
        WireBoundingBox {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 4.0,
        }
    }

    fn word(text: &str, confidence: Option<f32>) -> WireWord {
        WireWord {
            text: text.to_owned(),
            confidence,
            bbox: bbox(),
        }
    }

    fn response(blocks: Vec<WireBlock>, page_number: Option<u32>) -> OcrResponse {
        WireOcrResponse {
            pages: vec![WirePage {
                page_number,
                blocks,
            }],
        }
        .decode()
    }

    /// Words are what a populated response emits, and they carry the
    /// confidence the coarser levels have none of.
    #[test]
    fn emits_words_when_present() {
        let regions = response(
            vec![WireBlock {
                text: "hello world".to_owned(),
                bbox: bbox(),
                lines: vec![WireLine {
                    text: "hello world".to_owned(),
                    bbox: bbox(),
                    words: vec![word("hello", Some(0.9)), word("world", Some(0.8))],
                }],
            }],
            Some(1),
        )
        .regions;

        assert_eq!(regions.len(), 2);
        assert_eq!(regions[0].text, "hello");
        assert!(regions[0].confidence.is_some());
    }

    /// A line with text but no words is emitted itself: the contract
    /// requires `text` while defaulting `words` to empty, so discarding it
    /// would lose recognised content.
    #[test]
    fn falls_back_to_the_line_when_it_has_no_words() {
        let regions = response(
            vec![WireBlock {
                text: "a line".to_owned(),
                bbox: bbox(),
                lines: vec![WireLine {
                    text: "a line".to_owned(),
                    bbox: bbox(),
                    words: Vec::new(),
                }],
            }],
            Some(1),
        )
        .regions;

        assert_eq!(regions.len(), 1);
        assert_eq!(regions[0].text, "a line");
        // Only words carry a confidence, so a fallback region has none
        // rather than inheriting a number nobody reported.
        assert!(regions[0].confidence.is_none());
    }

    /// Same for a block whose `lines` is empty.
    #[test]
    fn falls_back_to_the_block_when_it_has_no_lines() {
        let regions = response(
            vec![WireBlock {
                text: "a block".to_owned(),
                bbox: bbox(),
                lines: Vec::new(),
            }],
            Some(2),
        )
        .regions;

        assert_eq!(regions.len(), 1);
        assert_eq!(regions[0].text, "a block");
        assert_eq!(regions[0].region.page, Some(2));
    }

    /// No pages decodes to no regions rather than failing.
    #[test]
    fn empty_response_decodes_to_no_regions() {
        assert!(
            WireOcrResponse { pages: Vec::new() }
                .decode()
                .regions
                .is_empty()
        );
    }
}
