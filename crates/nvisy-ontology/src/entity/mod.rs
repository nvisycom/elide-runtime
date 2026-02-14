//! Sensitive-data entity types and detection metadata.
//!
//! An [`Entity`] represents a single occurrence of sensitive data detected
//! within a document. Entities are produced by detection actions and consumed
//! by redaction and audit stages of the pipeline.

mod document;
mod location;
mod model;
mod selector;

pub use document::DocumentType;
pub use location::{
    AudioLocation, BoundingBox, EntityLocation, ImageLocation, TabularLocation,
    TextLocation, TimeSpan, VideoLocation,
};
pub use model::{ModelInfo, ModelKind};
pub use selector::EntitySelector;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use nvisy_core::path::ContentSource;

/// Category of sensitive data an entity belongs to.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "jsonschema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum EntityCategory {
    /// Personally Identifiable Information (names, SSNs, addresses, etc.).
    Pii,
    /// Protected Health Information (HIPAA-regulated data).
    Phi,
    /// Financial data (credit card numbers, bank accounts, etc.).
    Financial,
    /// Secrets and credentials (API keys, passwords, tokens).
    Credentials,
    /// Legal documents and privileged communications.
    Legal,
    /// Biometric data (fingerprints, iris scans, voiceprints).
    Biometric,
    /// User-defined or plugin-specific category.
    Custom(String),
}

/// Method used to detect a sensitive entity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "jsonschema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum DetectionMethod {
    /// Regular expression pattern matching.
    Regex,
    /// Checksum or Luhn-algorithm validation.
    Checksum,
    /// Lookup in a known-value dictionary.
    Dictionary,
    /// Named-entity recognition via AI model.
    Ner,
    /// Contextual NLP analysis (discourse-level understanding).
    ContextualNlp,
    /// OCR text extraction with bounding boxes.
    Ocr,
    /// Face detection in images or video frames.
    FaceDetection,
    /// Object detection in images or video frames.
    ObjectDetection,
    /// Entity detection from speech transcription.
    SpeechTranscript,
    /// Multiple methods combined to produce a single detection.
    Composite,
    /// User-provided annotations.
    Manual,
}

/// A detected sensitive data occurrence within a document.
///
/// Entities are produced by detection actions (regex, NER, checksum, etc.)
/// and later consumed by redaction and audit actions.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "jsonschema", derive(schemars::JsonSchema))]
pub struct Entity {
    /// Content source identity and lineage.
    #[serde(flatten)]
    pub source: ContentSource,
    /// Broad classification of the sensitive data.
    pub category: EntityCategory,
    /// Specific type label (e.g. `"ssn"`, `"email"`, `"credit_card"`).
    pub entity_type: String,
    /// The matched text or value.
    pub value: String,
    /// How this entity was detected.
    pub detection_method: DetectionMethod,
    /// Detection confidence score in the range `[0.0, 1.0]`.
    pub confidence: f64,
    /// Where this entity was found in the source content.
    pub location: EntityLocation,
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
        entity_type: impl Into<String>,
        value: impl Into<String>,
        detection_method: DetectionMethod,
        confidence: f64,
        location: EntityLocation,
    ) -> Self {
        Self {
            source: ContentSource::new(),
            category,
            entity_type: entity_type.into(),
            value: value.into(),
            detection_method,
            confidence,
            location,
            language: None,
            model: None,
            metadata: None,
        }
    }

    /// Set the parent source for lineage tracking.
    pub fn with_parent(mut self, parent: &ContentSource) -> Self {
        self.source = self.source.with_parent(parent);
        self
    }
}
