//! Context entry types for reference data.

use jiff::Timestamp;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::analytic::AnalyticVariant;
use super::biometric::BiometricVariant;
use super::document::DocumentVariant;
use super::geospatial::GeospatialVariant;
use super::reference::ReferenceVariant;
use super::temporal::TemporalVariant;

/// Top-level domain classification for context reference data.
///
/// Each domain contains a nested enum of specific variants,
/// keeping modality and semantic purpose cleanly separated.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "domain", content = "data", rename_all = "snake_case")]
#[non_exhaustive]
pub enum ContextEntryData {
    /// Identity verification via biological traits.
    Biometric(BiometricVariant),
    /// Geographic regions and addresses.
    Geospatial(GeospatialVariant),
    /// Computed representations for similarity search and pattern matching.
    Analytic(AnalyticVariant),
    /// Raw data for direct comparison against input.
    Reference(ReferenceVariant),
    /// Date and time-based matching.
    Temporal(TemporalVariant),
    /// Document templates and handwritten signatures.
    Document(DocumentVariant),
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
    /// When this entry was created.
    #[schemars(with = "String")]
    pub created_at: Timestamp,
    /// When this entry should stop being used.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<String>")]
    pub expires_at: Option<Timestamp>,
    /// Semantically typed payload.
    #[serde(flatten)]
    pub data: ContextEntryData,
}

impl ContextEntry {
    /// Create a new context entry with a generated UUID and current timestamp.
    pub fn new(data: ContextEntryData) -> Self {
        Self {
            id: Uuid::new_v4(),
            label: None,
            created_at: Timestamp::now(),
            expires_at: None,
            data,
        }
    }

    /// Set a human-readable label on this entry.
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Set an expiration timestamp on this entry.
    pub fn with_expires_at(mut self, expires_at: Timestamp) -> Self {
        self.expires_at = Some(expires_at);
        self
    }
}
