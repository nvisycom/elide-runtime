//! Sensitive-data entity types and detection metadata.
//!
//! An [`Entity`] represents a single occurrence of sensitive data detected
//! within a document.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use strum::Display;
use uuid::Uuid;

use nvisy_core::data::{EntityCategory, EntityKind};
use nvisy_core::path::ContentSource;

use super::location::Location;
use super::model::ModelInfo;

/// Method used to detect a sensitive entity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Display, Serialize, Deserialize)]
#[derive(schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum DetectionMethod {
    // A. Deterministic / pattern-based
    /// Regular expression pattern matching.
    Regex,
    /// Lookup in a known-value dictionary.
    Dictionary,

    // B. ML / NLP
    /// Named-entity recognition via AI model.
    Ner,
    /// Contextual NLP analysis (discourse-level understanding).
    ContextualNlp,

    // C. Computer vision
    /// OCR text extraction with bounding boxes.
    Ocr,
    /// Face detection in images or video frames.
    FaceDetection,
    /// Object detection in images or video frames.
    ObjectDetection,

    // D. Audio
    /// Entity detection from speech transcription.
    SpeechTranscript,
    /// Speaker-identified audio segment for redaction.
    SpeakerRedaction,

    // Meta
    /// Multiple methods combined to produce a single detection.
    Composite,
    /// User-provided annotations.
    Manual,
}

/// A detected sensitive data occurrence within a document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entity {
    /// Content source identity and lineage.
    #[serde(flatten)]
    pub source: ContentSource,
    /// Broad classification of the sensitive data.
    pub category: EntityCategory,
    /// Specific entity kind (e.g. `GovernmentId`, `EmailAddress`, `PaymentCard`).
    #[serde(rename = "entity_type")]
    pub entity_kind: EntityKind,
    /// The matched text or value.
    pub value: String,
    /// How this entity was detected.
    pub detection_method: DetectionMethod,
    /// Detection confidence score in the range `[0.0, 1.0]`.
    pub confidence: f64,
    /// Modality-specific location of the entity.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<Location>,
    /// BCP-47 language tag of the detected content.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    /// Detection model that produced this entity.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<ModelInfo>,
    /// Additional unstructured metadata.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Map<String, Value>>,
}

impl Entity {
    /// Create a new entity with the given detection details.
    pub fn new(
        category: EntityCategory,
        entity_kind: EntityKind,
        value: impl Into<String>,
        detection_method: DetectionMethod,
        confidence: f64,
    ) -> Self {
        Self {
            source: ContentSource::new(),
            category,
            entity_kind,
            value: value.into(),
            detection_method,
            confidence,
            location: None,
            language: None,
            model: None,
            metadata: None,
        }
    }

    /// Set the modality-specific location on this entity.
    pub fn with_location(mut self, location: Location) -> Self {
        self.location = Some(location);
        self
    }

    /// Set the parent source for lineage tracking.
    pub fn with_parent(mut self, parent: &ContentSource) -> Self {
        self.source = self.source.with_parent(parent);
        self
    }

    /// Copy the location from another entity.
    pub fn copy_location_from(&mut self, other: &Self) {
        self.location = other.location.clone();
    }
}

/// The output of a detection pass over a single content source.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectionOutput {
    /// Content source identity and lineage.
    #[serde(flatten)]
    pub source: ContentSource,
    /// Entities detected in the content.
    pub entities: Vec<Entity>,
    /// Identifier of the policy that governed detection.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub policy_id: Option<Uuid>,
    /// Processing time in milliseconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
}
