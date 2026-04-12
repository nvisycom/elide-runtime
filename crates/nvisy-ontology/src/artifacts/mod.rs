//! Modality-specific processing artifacts.
//!
//! [`ContentArtifacts`] stores intermediate data produced during
//! pipeline processing (OCR results, transcriptions, etc.), organized
//! by the content's modality.

mod audio;
mod image;
mod rich;
mod tabular;
mod text;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub use self::audio::{AudioArtifacts, Transcription};
pub use self::image::{
    ImageArtifacts, OcrBlock, OcrBlockKind, OcrLine, OcrPage, OcrWord,
};
pub use self::rich::RichArtifacts;
pub use self::tabular::TabularArtifacts;
pub use self::text::TextArtifacts;

/// Modality-specific processing artifacts.
///
/// Each variant matches a content modality and holds only the artifact
/// types that make sense for that modality (e.g. transcription is only
/// available on audio, OCR only on image/rich).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "modality", rename_all = "snake_case")]
pub enum ContentArtifacts {
    /// Artifacts from text content processing.
    Text(TextArtifacts),
    /// Artifacts from image content processing.
    Image(ImageArtifacts),
    /// Artifacts from audio content processing.
    Audio(AudioArtifacts),
    /// Artifacts from tabular content processing.
    Tabular(TabularArtifacts),
    /// Artifacts from rich document (PDF, DOCX) processing.
    Rich(RichArtifacts),
}

impl ContentArtifacts {
    /// Create empty text artifacts.
    pub fn text() -> Self {
        Self::Text(TextArtifacts::default())
    }

    /// Create empty image artifacts.
    pub fn image() -> Self {
        Self::Image(ImageArtifacts::default())
    }

    /// Create empty audio artifacts.
    pub fn audio() -> Self {
        Self::Audio(AudioArtifacts::default())
    }

    /// Create empty tabular artifacts.
    pub fn tabular() -> Self {
        Self::Tabular(TabularArtifacts::default())
    }

    /// Create empty rich artifacts.
    pub fn rich() -> Self {
        Self::Rich(RichArtifacts::default())
    }

    /// Access image artifacts, if this is an image or rich variant.
    pub fn as_image(&self) -> Option<&ImageArtifacts> {
        match self {
            Self::Image(a) => Some(a),
            Self::Rich(a) => Some(&a.image),
            _ => None,
        }
    }

    /// Access image artifacts mutably, if this is an image or rich variant.
    pub fn as_image_mut(&mut self) -> Option<&mut ImageArtifacts> {
        match self {
            Self::Image(a) => Some(a),
            Self::Rich(a) => Some(&mut a.image),
            _ => None,
        }
    }

    /// Access audio artifacts, if this is an audio variant.
    pub fn as_audio(&self) -> Option<&AudioArtifacts> {
        match self {
            Self::Audio(a) => Some(a),
            _ => None,
        }
    }

    /// Access audio artifacts mutably, if this is an audio variant.
    pub fn as_audio_mut(&mut self) -> Option<&mut AudioArtifacts> {
        match self {
            Self::Audio(a) => Some(a),
            _ => None,
        }
    }
}
