//! Redaction mapping artifact.
//!
//! A [`RedactionMap`] records the correspondence between original values and
//! their redacted replacements across all modalities. Each entry embeds a
//! [`Location`] for modality-specific positioning and is flagged as reversible
//! or not, enabling reconstruction of the original from the redacted output
//! when authorized.

use nvisy_core::content::ContentSource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::entity::Location;

/// A single entry in a [`RedactionMap`], linking an entity and its redaction
/// to a modality-specific location.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RedactionMapEntry {
    /// Identifier of the entity that was redacted.
    pub entity_id: Uuid,
    /// Identifier of the redaction decision.
    pub redaction_id: Uuid,
    /// Modality-specific location of the redaction.
    pub location: Location,
    /// The original sensitive value (text modality; `None` for image/audio).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original: Option<String>,
    /// The replacement string used (text modality; `None` for image/audio).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub replacement: Option<String>,
    /// Redaction method applied (e.g. `"blur"`, `"silence"`; `None` for text).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
    /// Whether the original can be reconstructed from this mapping.
    pub reversible: bool,
}

/// A standalone artifact mapping original values to redacted replacements.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RedactionMap {
    /// Content source this map belongs to.
    pub source: ContentSource,
    /// Identifier of the pipeline run that produced this map.
    pub run_id: Uuid,
    /// Ordered list of mapping entries.
    pub entries: Vec<RedactionMapEntry>,
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

    /// Append an entry to the map.
    pub fn push(&mut self, entry: RedactionMapEntry) {
        self.entries.push(entry);
    }

    /// Number of entries in the map.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` if the map contains no entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Returns an iterator over entries that are flagged as reversible.
    pub fn reversible_entries(&self) -> impl Iterator<Item = &RedactionMapEntry> {
        self.entries.iter().filter(|entry| entry.reversible)
    }
}
