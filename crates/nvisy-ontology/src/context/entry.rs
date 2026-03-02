//! Context entry types for reference data.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumString};
use uuid::Uuid;

use nvisy_core::math::{BoundingBox, TimeSpan};
use nvisy_core::path::ContentSource;

/// Classifies the kind of reference data held by a [`ContextEntry`].
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Display, EnumString, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum ContextKind {
    /// Names, identifiers, or phrases to match.
    TextValue,
    /// Regex or glob patterns.
    Pattern,
    /// Reference face image for matching.
    FaceImage,
    /// Reference voice sample for speaker identification.
    VoiceSample,
    /// Pre-computed embedding vector.
    Embedding,
    /// Date or date-range to match.
    DateValue,
    /// Keyword tag for routing.
    Tag,
}

/// Modality-specific payload for a [`ContextEntry`].
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContextEntryData {
    /// Textual reference values.
    Text {
        /// One or more text values to match.
        values: Vec<String>,
    },
    /// Image reference (face, object, etc.).
    Image {
        /// Source pointer to the reference image.
        image_source: ContentSource,
        /// Optional sub-region within the image.
        #[serde(skip_serializing_if = "Option::is_none")]
        region: Option<BoundingBox>,
    },
    /// Audio reference (voice sample, etc.).
    Audio {
        /// Source pointer to the reference audio.
        audio_source: ContentSource,
        /// Optional time segment within the audio.
        #[serde(skip_serializing_if = "Option::is_none")]
        segment: Option<TimeSpan>,
    },
    /// Pre-computed embedding vector.
    Embedding {
        /// The embedding vector values.
        vector: Vec<f64>,
        /// Dimensionality of the vector.
        dimensions: u32,
    },
}

/// A single reference-data entry within a [`Context`](super::Context).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ContextEntry {
    /// Unique identifier for this entry.
    pub id: Uuid,
    /// Classification of this entry.
    pub kind: ContextKind,
    /// Human-readable label.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// Modality-specific payload.
    pub data: ContextEntryData,
}

impl ContextEntry {
    /// Create a new context entry with a generated UUID.
    pub fn new(kind: ContextKind, data: ContextEntryData) -> Self {
        Self {
            id: Uuid::new_v4(),
            kind,
            label: None,
            data,
        }
    }

    /// Set a human-readable label on this entry.
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }
}
