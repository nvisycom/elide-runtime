//! Context entry types for reference data.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{AudioData, EmbeddingData, ImageData, TextData};

/// Semantically typed reference-data payload for a [`ContextEntry`].
///
/// Each variant combines a semantic purpose (what the data *means*) with a
/// modality-specific payload (what the data *looks like*).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ContextEntryData {
    // Text
    /// Names, identifiers, or phrases to match.
    TextValue(TextData),
    /// Regex or glob patterns.
    Pattern(TextData),
    /// Date or date-range to match.
    DateValue(TextData),
    /// Keyword tag for routing.
    Tag(TextData),
    /// API keys, tokens, or known secret patterns.
    Credential(TextData),

    // Image
    /// Reference face image for matching.
    FaceImage(ImageData),
    /// Reference object or scene image for matching.
    ObjectImage(ImageData),
    /// Brand or logo reference image.
    Logo(ImageData),
    /// Reference document template (ID card, passport, form, etc.).
    Document(ImageData),
    /// Handwritten signature reference.
    Signature(ImageData),

    // Audio
    /// Reference voice sample for speaker identification.
    VoiceSample(AudioData),
    /// Spoken keyword or phrase for audio spotting.
    SpokenKeyword(AudioData),

    // Embedding
    /// Pre-computed embedding vector.
    Embedding(EmbeddingData),
}

/// A single reference-data entry within a [`Context`](super::Context).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ContextEntry {
    /// Unique identifier for this entry.
    pub id: Uuid,
    /// Human-readable label.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// Semantically typed payload.
    #[serde(flatten)]
    pub data: ContextEntryData,
}

impl ContextEntry {
    /// Create a new context entry with a generated UUID.
    pub fn new(data: ContextEntryData) -> Self {
        Self {
            id: Uuid::new_v4(),
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
