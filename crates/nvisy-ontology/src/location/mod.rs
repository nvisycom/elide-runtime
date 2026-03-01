//! Modality-specific entity location types.

mod layout_kind;
mod text_level;

pub use layout_kind::LayoutKind;
pub use text_level::TextLevel;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use nvisy_core::math::{BoundingBox, TimeSpan};

/// Location of an entity within text content.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct TextLocation {
    /// Byte or character offset where the entity starts.
    pub start_offset: usize,
    /// Byte or character offset where the entity ends.
    pub end_offset: usize,
    /// Start offset of the surrounding context window for redaction.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_start_offset: Option<usize>,
    /// End offset of the surrounding context window for redaction.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_end_offset: Option<usize>,
    /// Identifier of the document element containing this entity.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub element_id: Option<String>,
    /// 1-based page number where the entity was found.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_number: Option<u32>,
}

impl TextLocation {
    /// Returns `true` if this text location overlaps with `other`.
    pub fn overlaps(&self, other: &TextLocation) -> bool {
        self.start_offset < other.end_offset && other.start_offset < self.end_offset
    }
}

/// Location of an entity within an image.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ImageLocation {
    /// Bounding box of the entity in the image.
    pub bounding_box: BoundingBox,
    /// Links this entity to a specific image document.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_id: Option<Uuid>,
    /// 1-based page number (for multi-page documents like PDFs).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_number: Option<u32>,
}

/// Location of an entity within tabular data.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TabularLocation {
    /// Row index (0-based).
    pub row_index: usize,
    /// Column index (0-based).
    pub column_index: usize,
    /// Byte offset within the cell where the entity starts, if applicable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_offset: Option<usize>,
    /// Byte offset within the cell where the entity ends, if applicable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_offset: Option<usize>,
}

/// Location of an entity within an audio stream.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AudioLocation {
    /// Time interval of the entity.
    pub time_span: TimeSpan,
    /// Speaker identifier from diarization.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speaker_id: Option<String>,
    /// Links this entity to a specific audio document.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio_id: Option<Uuid>,
}

/// Location of an entity within a video stream.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct VideoLocation {
    /// Bounding box of the entity in the frame.
    pub bounding_box: BoundingBox,
    /// 0-based frame number where the entity was detected.
    pub frame_number: u64,
    /// Time interval of the entity in the video.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time_span: Option<TimeSpan>,
    /// Tracking identifier for an entity across multiple frames.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub track_id: Option<String>,
    /// Speaker identifier from diarization (for audio track).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speaker_id: Option<String>,
}

/// A modality-specific location for a detected entity.
///
/// Exactly one variant is set per entity, enforcing the invariant that
/// an entity exists in a single modality.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Location {
    /// Entity found in text content.
    Text(TextLocation),
    /// Entity found in an image.
    Image(ImageLocation),
    /// Entity found in tabular data.
    Tabular(TabularLocation),
    /// Entity found in audio.
    Audio(AudioLocation),
    /// Entity found in video.
    Video(VideoLocation),
}

impl Location {
    /// If this is a text location, return a reference to it.
    pub fn as_text(&self) -> Option<&TextLocation> {
        match self {
            Self::Text(loc) => Some(loc),
            _ => None,
        }
    }

    /// If this is an image location, return a reference to it.
    pub fn as_image(&self) -> Option<&ImageLocation> {
        match self {
            Self::Image(loc) => Some(loc),
            _ => None,
        }
    }

    /// If this is a tabular location, return a reference to it.
    pub fn as_tabular(&self) -> Option<&TabularLocation> {
        match self {
            Self::Tabular(loc) => Some(loc),
            _ => None,
        }
    }

    /// If this is an audio location, return a reference to it.
    pub fn as_audio(&self) -> Option<&AudioLocation> {
        match self {
            Self::Audio(loc) => Some(loc),
            _ => None,
        }
    }

    /// If this is a video location, return a reference to it.
    pub fn as_video(&self) -> Option<&VideoLocation> {
        match self {
            Self::Video(loc) => Some(loc),
            _ => None,
        }
    }
}
