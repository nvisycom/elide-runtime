//! Redaction mapping artifact.
//!
//! A [`RedactionMap`] records the correspondence between original values and
//! their redacted replacements across all modalities. Each entry is flagged
//! as reversible or not, enabling reconstruction of the original from the
//! redacted output when authorized.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use nvisy_core::math::{BoundingBox, TimeSpan};
use nvisy_core::path::ContentSource;

/// Mapping entry for a text-modality redaction.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TextMapEntry {
    /// Start byte offset in the original content.
    pub start_offset: usize,
    /// End byte offset in the original content.
    pub end_offset: usize,
    /// The original sensitive value.
    pub original: String,
    /// The replacement string used.
    pub replacement: String,
}

/// Mapping entry for an image-modality redaction.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ImageMapEntry {
    /// Region that was redacted.
    pub bounding_box: BoundingBox,
    /// Page number for multi-page documents.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_number: Option<u32>,
    /// Description of the original content.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_description: Option<String>,
    /// Redaction method applied (e.g. `"blur"`, `"block"`).
    pub method: String,
}

/// Mapping entry for an audio-modality redaction.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AudioMapEntry {
    /// Time segment that was redacted.
    pub time_span: TimeSpan,
    /// Transcript of the original audio segment.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_transcript: Option<String>,
    /// Redaction method applied (e.g. `"silence"`, `"remove"`).
    pub method: String,
}

/// Modality-specific redaction mapping.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RedactionMapEntry {
    /// Text-modality mapping.
    Text(TextMapEntry),
    /// Image-modality mapping.
    Image(ImageMapEntry),
    /// Audio-modality mapping.
    Audio(AudioMapEntry),
}

/// A single item in a [`RedactionMap`], linking an entity and its redaction
/// to a modality-specific mapping.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RedactionMapItem {
    /// Identifier of the entity that was redacted.
    pub entity_id: Uuid,
    /// Identifier of the redaction record.
    pub redaction_id: Uuid,
    /// Modality-specific mapping details.
    pub mapping: RedactionMapEntry,
    /// Whether the original can be reconstructed from this mapping.
    pub reversible: bool,
}

/// A standalone artifact mapping original values to redacted replacements.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RedactionMap {
    /// Content source this map belongs to.
    #[serde(flatten)]
    pub source: ContentSource,
    /// Identifier of the pipeline run that produced this map.
    pub run_id: Uuid,
    /// Ordered list of mapping items.
    pub entries: Vec<RedactionMapItem>,
}

impl RedactionMap {
    /// Create a new empty redaction map for the given source and run.
    pub fn new(source: ContentSource, run_id: Uuid) -> Self {
        Self {
            source,
            run_id,
            entries: Vec::new(),
        }
    }

    /// Append an item to the map.
    pub fn push(&mut self, item: RedactionMapItem) {
        self.entries.push(item);
    }

    /// Number of items in the map.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` if the map contains no items.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Returns an iterator over entries that are flagged as reversible.
    pub fn reversible_entries(&self) -> impl Iterator<Item = &RedactionMapItem> {
        self.entries.iter().filter(|item| item.reversible)
    }
}
