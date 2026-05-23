//! OCR output types.
//!
//! The structural types ([`Word`], [`Line`], [`Block`], [`Page`]) are
//! re-exported from [`artifacts`]. [`ImageOutput`]
//! wraps them with a [`ContentSource`] for provenance tracking.
//!
//! [`artifacts`]: nvisy_ontology::artifacts

use nvisy_core::content::ContentSource;
use nvisy_ontology::artifacts::{Block, Line, Page, Word};

/// Complete OCR output for one image/document.
///
/// Groups detected text into a hierarchical tree of
/// [`Page`] → [`Block`] → [`Line`] → [`Word`], together with a
/// [`ContentSource`] derived from the input image for provenance tracking.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ImageOutput {
    /// Provenance: derived from the input's [`ContentSource`].
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
