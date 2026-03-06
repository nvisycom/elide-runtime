//! Sensitive-data entity types and detection metadata.
//!
//! An [`Entity`] represents a single occurrence of sensitive data detected
//! within a document.

mod annotation;
mod category;
mod kind;
mod location;
mod model;
mod selector;
mod sensitivity;

use std::time::Duration;

pub use annotation::{Annotation, AnnotationKind, AnnotationLabel, AnnotationScope};
pub use category::EntityCategory;
pub use kind::EntityKind;
pub use location::{AudioLocation, ImageLocation, Location, TabularLocation, TextLocation};
pub use model::{ModelInfo, ModelKind};
use nvisy_core::path::ContentSource;
use schemars::JsonSchema;
pub use selector::EntitySelector;
pub use sensitivity::EntitySensitivity;
use serde::{Deserialize, Serialize};
use serde_with::{DurationMicroSeconds, serde_as};
use strum::{Display, EnumString};
use uuid::Uuid;

/// Method used to detect a sensitive entity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[derive(Display, EnumString, Serialize, Deserialize, JsonSchema)]
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
    /// Face detection in images.
    FaceDetection,
    /// Object detection in images.
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
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Entity {
    /// Content source identity and lineage.
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
}

impl Entity {
    /// The unique identifier for this entity (delegates to `source.as_uuid()`).
    pub fn id(&self) -> Uuid {
        self.source.as_uuid()
    }

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

    /// Set the BCP-47 language tag.
    pub fn with_language(mut self, language: impl Into<String>) -> Self {
        self.language = Some(language.into());
        self
    }

    /// Set the detection model.
    pub fn with_model(mut self, model: ModelInfo) -> Self {
        self.model = Some(model);
        self
    }

    /// Copy the location from another entity.
    pub fn copy_location_from(mut self, other: &Self) -> Self {
        self.location = other.location.clone();
        self
    }
}

/// The output of a detection pass over a single content source.
#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DetectionOutput {
    /// Content source identity and lineage.
    pub source: ContentSource,
    /// Entities detected in the content.
    pub entities: Vec<Entity>,
    /// Identifier of the policy that governed detection.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub policy_id: Option<Uuid>,
    /// Processing time.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde_as(as = "Option<DurationMicroSeconds>")]
    #[schemars(with = "Option<u64>")]
    pub duration: Option<Duration>,
    /// Non-fatal errors or warnings encountered during detection.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub errors: Vec<String>,
}

impl DetectionOutput {
    /// The unique identifier for this detection output (delegates to `source.as_uuid()`).
    pub fn id(&self) -> Uuid {
        self.source.as_uuid()
    }

    /// Create a new detection output for the given source.
    pub fn new(source: ContentSource, entities: Vec<Entity>) -> Self {
        Self {
            source,
            entities,
            policy_id: None,
            duration: None,
            errors: Vec::new(),
        }
    }

    /// Set the policy identifier.
    pub fn with_policy_id(mut self, policy_id: Uuid) -> Self {
        self.policy_id = Some(policy_id);
        self
    }

    /// Set the processing duration.
    pub fn with_duration(mut self, duration: Duration) -> Self {
        self.duration = Some(duration);
        self
    }
}
