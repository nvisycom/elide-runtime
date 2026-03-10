//! OCR output types.

use nvisy_core::math::{BoundingBox, Polygon};
use nvisy_core::content::ContentSource;
use serde::{Deserialize, Serialize};

/// A single word detected by OCR.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Word {
    /// Recognised text content.
    pub text: String,
    /// Confidence score (0.0..=1.0), if the backend provides one.
    pub confidence: Option<f64>,
    /// Axis-aligned bounding box in pixel coordinates.
    pub bbox: BoundingBox,
    /// Polygon vertices for rotated or skewed text regions.
    pub polygon: Option<Polygon>,
}

/// A line of text: ordered sequence of words.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Line {
    /// Concatenated text from all words in this line.
    pub text: String,
    /// Line-level confidence, if the provider gives one.
    pub confidence: Option<f64>,
    /// Axis-aligned bounding box enclosing the line.
    pub bbox: BoundingBox,
    /// Polygon vertices for the line region.
    pub polygon: Option<Polygon>,
    /// Words in reading order.
    pub words: Vec<Word>,
}

/// Classification of a block region.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Block {
    /// Concatenated text from all lines in this block.
    pub text: String,
    /// Block-level confidence, if available.
    pub confidence: Option<f64>,
    /// Axis-aligned bounding box enclosing the block.
    pub bbox: BoundingBox,
    /// Polygon vertices for the block region.
    pub polygon: Option<Polygon>,
    /// Classification of this block.
    pub kind: BlockKind,
    /// Lines in reading order.
    pub lines: Vec<Line>,
}

/// A single page of OCR results.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Page {
    /// 1-based page number.
    pub page_number: u32,
    /// Page width in pixels, when known.
    pub width: Option<f64>,
    /// Page height in pixels, when known.
    pub height: Option<f64>,
    /// Blocks in reading order.
    pub blocks: Vec<Block>,
}

/// Complete OCR output for one image/document.
///
/// Groups detected text into a hierarchical tree of
/// [`Page`] → [`Block`] → [`Line`] → [`Word`], together with a
/// [`ContentSource`] derived from the input image for provenance tracking.
///
/// [`ContentSource`]: nvisy_core::content::ContentSource
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageOutput {
    /// Provenance: derived from the input's [`ContentSource`].
    ///
    /// [`ContentSource`]: nvisy_core::content::ContentSource
    pub source: ContentSource,
    /// Pages of OCR results.
    pub pages: Vec<Page>,
}

impl ImageOutput {
    /// Create an empty output with the given source.
    pub fn new(source: ContentSource) -> Self {
        Self {
            source,
            pages: Vec::new(),
        }
    }

    /// Number of pages.
    pub fn len(&self) -> usize {
        self.pages.len()
    }

    /// Returns `true` if no pages or no words were detected.
    pub fn is_empty(&self) -> bool {
        self.pages.is_empty() || self.words().next().is_none()
    }

    /// Flat iterator over all words across all pages/blocks/lines.
    pub fn words(&self) -> impl Iterator<Item = &Word> {
        self.pages
            .iter()
            .flat_map(|p| &p.blocks)
            .flat_map(|b| &b.lines)
            .flat_map(|l| &l.words)
    }

    /// Flat iterator over all lines.
    pub fn lines(&self) -> impl Iterator<Item = &Line> {
        self.pages
            .iter()
            .flat_map(|p| &p.blocks)
            .flat_map(|b| &b.lines)
    }

    /// Flat iterator over all blocks.
    pub fn blocks(&self) -> impl Iterator<Item = &Block> {
        self.pages.iter().flat_map(|p| &p.blocks)
    }

    /// Full extracted text (pages joined by `\n\n`).
    pub fn full_text(&self) -> String {
        self.pages
            .iter()
            .map(|p| {
                p.blocks
                    .iter()
                    .map(|b| b.text.as_str())
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .collect::<Vec<_>>()
            .join("\n\n")
    }

    /// Total word count across all pages.
    pub fn word_count(&self) -> usize {
        self.words().count()
    }
}
