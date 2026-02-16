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
    AudioLocation, BoundingBox, BoundingBoxU32, ImageLocation, TabularLocation,
    TextLocation, TimeSpan, VideoLocation,
};
pub use model::{ModelInfo, ModelKind};
pub use selector::EntitySelector;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use nvisy_core::path::ContentSource;

/// Category of sensitive data an entity belongs to.
#[derive(Debug, Clone, PartialEq, Eq, Hash, strum::Display, Serialize, Deserialize)]
#[cfg_attr(feature = "jsonschema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
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
    #[strum(to_string = "{0}")]
    Custom(String),
}

/// Method used to detect a sensitive entity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, strum::Display, Serialize, Deserialize)]
#[cfg_attr(feature = "jsonschema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
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
    /// Text location, if this entity was found in text content.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_location: Option<TextLocation>,
    /// Image location, if this entity was found in an image.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_location: Option<ImageLocation>,
    /// Tabular location, if this entity was found in tabular data.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tabular_location: Option<TabularLocation>,
    /// Audio location, if this entity was found in audio.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio_location: Option<AudioLocation>,
    /// Video location, if this entity was found in video.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub video_location: Option<VideoLocation>,
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
    ) -> Self {
        Self {
            source: ContentSource::new(),
            category,
            entity_type: entity_type.into(),
            value: value.into(),
            detection_method,
            confidence,
            text_location: None,
            image_location: None,
            tabular_location: None,
            audio_location: None,
            video_location: None,
            language: None,
            model: None,
            metadata: None,
        }
    }

    /// Set a text location on this entity.
    pub fn with_text_location(mut self, location: TextLocation) -> Self {
        self.text_location = Some(location);
        self
    }

    /// Set an image location on this entity.
    pub fn with_image_location(mut self, location: ImageLocation) -> Self {
        self.image_location = Some(location);
        self
    }

    /// Set a tabular location on this entity.
    pub fn with_tabular_location(mut self, location: TabularLocation) -> Self {
        self.tabular_location = Some(location);
        self
    }

    /// Set an audio location on this entity.
    pub fn with_audio_location(mut self, location: AudioLocation) -> Self {
        self.audio_location = Some(location);
        self
    }

    /// Set a video location on this entity.
    pub fn with_video_location(mut self, location: VideoLocation) -> Self {
        self.video_location = Some(location);
        self
    }

    /// Set the parent source for lineage tracking.
    pub fn with_parent(mut self, parent: &ContentSource) -> Self {
        self.source = self.source.with_parent(parent);
        self
    }

    /// Copy all location fields from another entity.
    pub fn copy_locations_from(&mut self, other: &Self) {
        self.text_location = other.text_location.clone();
        self.image_location = other.image_location.clone();
        self.tabular_location = other.tabular_location.clone();
        self.audio_location = other.audio_location.clone();
        self.video_location = other.video_location.clone();
    }
}
