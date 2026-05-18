//! Image-modality artifacts.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::primitive::{BoundingBox, Polygon};

/// A single word detected by OCR.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Word {
    /// Recognised text content.
    pub text: String,
    /// Confidence score (0.0..=1.0), if available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f64>,
    /// Axis-aligned bounding box in pixel coordinates.
    pub bbox: BoundingBox,
    /// Polygon vertices for rotated or skewed text regions.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub polygon: Option<Polygon>,
}

/// A line of text: ordered sequence of words.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Line {
    /// Concatenated text from all words in this line.
    pub text: String,
    /// Line-level confidence, if available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f64>,
    /// Axis-aligned bounding box enclosing the line.
    pub bbox: BoundingBox,
    /// Polygon vertices for the line region.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub polygon: Option<Polygon>,
    /// Words in reading order.
    pub words: Vec<Word>,
}

/// Classification of a block region.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    Serialize,
    Deserialize,
    JsonSchema
)]
#[serde(rename_all = "snake_case")]
pub enum BlockKind {
    /// Paragraph / prose.
    Text,
    /// Tabular content.
    Table,
    /// Figure / chart.
    Figure,
    /// Unclassified.
    Other,
}

/// A block (paragraph, table cell, figure caption, etc.).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Block {
    /// Concatenated text from all lines in this block.
    pub text: String,
    /// Block-level confidence, if available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f64>,
    /// Axis-aligned bounding box enclosing the block.
    pub bbox: BoundingBox,
    /// Polygon vertices for the block region.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub polygon: Option<Polygon>,
    /// Classification of this block.
    pub kind: BlockKind,
    /// Lines in reading order.
    pub lines: Vec<Line>,
}

/// OCR results for a single page.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Page {
    /// 1-based page number.
    pub page_number: u32,
    /// Page width in pixels, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<f64>,
    /// Page height in pixels, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<f64>,
    /// Blocks in reading order.
    pub blocks: Vec<Block>,
}

/// Artifacts produced during processing of image content.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ImageArtifacts {
    /// OCR results as a hierarchical page → block → line → word tree.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ocr_pages: Vec<Page>,
}
