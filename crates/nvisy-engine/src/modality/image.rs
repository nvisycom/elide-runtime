//! Image-modality document shape: [`ImageBlock`], [`ImageMetadata`],
//! [`PageDimensions`].

use nvisy_core::modality::{ImageExtraction, ImageLocation};
use nvisy_core::primitive::LanguageDetection;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::ModalityBlock;

/// Per-modality block payload for [`Image`]. Text-bearing variants
/// carry recognized text; per-word source spans live on the wrapping
/// [`Block<Image>`]. Non-textual variants ([`Figure`], [`Separator`],
/// [`Background`], [`Logo`]) carry no text. Every variant carries the
/// bounding `region` since image blocks are always spatially located.
///
/// [`Image`]: nvisy_core::modality::Image
/// [`Figure`]: Self::Figure
/// [`Separator`]: Self::Separator
/// [`Background`]: Self::Background
/// [`Logo`]: Self::Logo
/// [`Block<Image>`]: crate::document::Block
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum ImageBlock {
    /// A region of recognized text (paragraph, line, OCR text block).
    Text { region: ImageLocation, text: String },
    /// A heading.
    Heading { region: ImageLocation, text: String },
    /// A tabular region recognized in the image.
    Table { region: ImageLocation, text: String },
    /// A figure, illustration or photograph.
    Figure { region: ImageLocation },
    /// A separator (rule, line, divider).
    Separator { region: ImageLocation },
    /// A background element (watermark, fill, decoration).
    Background { region: ImageLocation },
    /// A logo or brand mark.
    Logo { region: ImageLocation },
}

impl ImageBlock {
    /// The image region this block occupies.
    pub fn region(&self) -> &ImageLocation {
        match self {
            Self::Text { region, .. }
            | Self::Heading { region, .. }
            | Self::Table { region, .. }
            | Self::Figure { region }
            | Self::Separator { region }
            | Self::Background { region }
            | Self::Logo { region } => region,
        }
    }

    /// Recognized text for text-bearing kinds, `None` for non-textual.
    pub fn text(&self) -> Option<&str> {
        match self {
            Self::Text { text, .. } | Self::Heading { text, .. } | Self::Table { text, .. } => {
                Some(text)
            }
            Self::Figure { .. }
            | Self::Separator { .. }
            | Self::Background { .. }
            | Self::Logo { .. } => None,
        }
    }
}

impl ModalityBlock for ImageBlock {
    fn scan_text(&self) -> Option<&str> {
        self.text()
    }
}

/// Document-level metadata for [`Document<Image>`].
///
/// [`Document<Image>`]: crate::document::Document
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ImageMetadata {
    /// How this document's image content was processed (OCR, scene
    /// text, object detection, layout analysis).
    pub extraction: ImageExtraction,
    /// Languages detected (or asserted) for the document content.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[schemars(skip)]
    pub languages: Vec<LanguageDetection>,
    /// Page dimensions for multi-page sources (PDFs, multi-image
    /// uploads). Empty for single-page sources.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pages: Vec<PageDimensions>,
}

impl From<ImageExtraction> for ImageMetadata {
    /// Build [`ImageMetadata`] carrying only the importer-known
    /// extraction tag. Languages and page dimensions start empty;
    /// downstream stages fill them in.
    fn from(extraction: ImageExtraction) -> Self {
        Self {
            extraction,
            languages: Vec::new(),
            pages: Vec::new(),
        }
    }
}

/// Dimensions of a single page in an image document.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PageDimensions {
    /// 1-based page number.
    pub number: u32,
    /// Page width in pixels.
    pub width: f64,
    /// Page height in pixels.
    pub height: f64,
}
