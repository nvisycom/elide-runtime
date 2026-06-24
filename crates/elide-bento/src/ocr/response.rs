//! Incoming wire types for the OCR `/recognize` endpoint.
//!
//! Mirrors `nvisy_core.ocr.v1.OcrResponse` from the inference
//! repository. The full upstream tree is
//! `Page -> Block -> Line -> Word`; elide's vocabulary collapses
//! lines into the parent block — [`WireOcrResponse::decode`]
//! flattens every word under its grandparent block. The
//! response-level `modelId`,
//! per-page `width`/`height`, per-block `kind`, and any rotated
//! polygons are deserialised-and-discarded for now.

use elide_core::modality::image::{ImageLocation, LayoutBlock, LayoutWord};
use elide_core::primitive::{BoundingBox, Confidence, Point};
use elide_ocr::backend::OcrResponse;
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
    // `kind` (text / table / figure / other) ignored: elide's
    // `LayoutBlock` does not yet model block kind. When upstream
    // grows it, surface here.
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct WireLine {
    #[serde(default)]
    pub words: Vec<WireWord>,
    // Per-line text + bbox are subsumed by the block's text + the
    // per-word geometry; elide does not model the intermediate line
    // layer today.
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct WireWord {
    pub text: String,
    pub confidence: Option<f32>,
    pub bbox: WireBoundingBox,
    // `polygon` ignored: rotated regions land on a future
    // `LayoutWord::polygon` field once elide grows one.
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct WireBoundingBox {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl From<WireBoundingBox> for BoundingBox {
    fn from(b: WireBoundingBox) -> Self {
        BoundingBox::new(
            Point::new(b.x, b.y),
            Point::new(b.x + b.width, b.y + b.height),
        )
    }
}

impl WireOcrResponse {
    /// Translate into the elide [`OcrResponse`] the backend trait
    /// expects. Flattens pages → blocks → words; every per-block
    /// word becomes a [`LayoutWord`] on the resulting
    /// [`LayoutBlock`].
    pub(super) fn decode(self) -> OcrResponse {
        let blocks = self
            .pages
            .into_iter()
            .flat_map(|page| {
                let page_number = page.page_number;
                page.blocks.into_iter().map(move |block| block.decode(page_number))
            })
            .collect();
        OcrResponse::new(blocks)
    }
}

impl WireBlock {
    fn decode(self, page_number: Option<u32>) -> LayoutBlock {
        let region = ImageLocation {
            bounding_box: self.bbox.into(),
            polygon: None,
            page: page_number,
        };
        let words: Vec<LayoutWord> = self
            .lines
            .into_iter()
            .flat_map(|line| line.words.into_iter())
            .map(|word| word.decode(page_number))
            .collect();
        LayoutBlock::new(region, self.text).with_words(words)
    }
}

impl WireWord {
    fn decode(self, page_number: Option<u32>) -> LayoutWord {
        let region = ImageLocation {
            bounding_box: self.bbox.into(),
            polygon: None,
            page: page_number,
        };
        let mut layout = LayoutWord::new(region, self.text);
        if let Some(c) = self.confidence {
            layout = layout.with_confidence(Confidence::clamped(c));
        }
        layout
    }
}
