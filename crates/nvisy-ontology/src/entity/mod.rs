//! Sensitive-data entity types and detection metadata.
//!
//! An [`Entity`] represents a single occurrence of sensitive data detected
//! within a document. [`Entities`] is the canonical collection type used
//! across the pipeline.

mod annotation;
mod category;
mod kind;
mod location;
mod method;
mod model;
mod output;
mod sensitivity;

use derive_more::{Deref, DerefMut, From, IntoIterator};
use nvisy_core::content::ContentSource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub use self::annotation::{Annotation, AnnotationKind, AnnotationLabel, AnnotationScope};
pub use self::category::EntityCategory;
pub use self::kind::EntityKind;
pub use self::location::{
    AudioLocation, ImageLocation, Location, Overlap, TabularLocation, TextLocation,
};
pub use self::method::{ExtractionMethod, RecognitionMethod, RefinementMethod};
pub use self::model::{ModelInfo, ModelKind};
pub use self::output::DetectionOutput;
pub use self::sensitivity::EntitySensitivity;

/// A detected sensitive data occurrence within a document.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
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
    /// How content was extracted from its source modality, ordered by application time.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extraction_methods: Vec<ExtractionMethod>,
    /// Techniques used to identify this entity, ordered by application time.
    pub recognition_methods: Vec<RecognitionMethod>,
    /// Post-detection refinements applied to this entity, ordered by application time.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub refinement_methods: Vec<RefinementMethod>,
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

    /// Create a new entity with the given recognition method and confidence.
    ///
    /// The `category` is derived from `entity_kind` via
    /// [`EntityKind::category()`] when not supplied explicitly.
    pub fn new(
        category: EntityCategory,
        entity_kind: EntityKind,
        value: impl Into<String>,
        recognition_method: RecognitionMethod,
        confidence: f64,
    ) -> Self {
        Self {
            source: ContentSource::new(),
            category,
            entity_kind,
            value: value.into(),
            extraction_methods: Vec::new(),
            recognition_methods: vec![recognition_method],
            refinement_methods: Vec::new(),
            confidence,
            location: None,
            language: None,
            model: None,
        }
    }

    /// Create a new entity, deriving the category from the entity kind.
    pub fn from_kind(
        entity_kind: EntityKind,
        value: impl Into<String>,
        recognition_method: RecognitionMethod,
        confidence: f64,
    ) -> Self {
        Self::new(
            entity_kind.category(),
            entity_kind,
            value,
            recognition_method,
            confidence,
        )
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

/// A collection of detected entities.
///
/// Wraps `Vec<Entity>` with transparent `Deref`/`DerefMut` access and
/// domain-specific filtering helpers. Used as the canonical entity
/// container in both operation I/O and [`DocumentEnvelope`].
///
/// [`DocumentEnvelope`]: https://docs.rs/nvisy-engine/latest/nvisy_engine/operation/struct.DocumentEnvelope.html
#[derive(Debug, Clone, Default, PartialEq)]
#[derive(Deref, DerefMut, From, IntoIterator)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
pub struct Entities(pub Vec<Entity>);

impl Entities {
    /// Create an empty collection.
    pub fn new() -> Self {
        Self(Vec::new())
    }

    /// Append an entity.
    pub fn push(&mut self, entity: Entity) {
        self.0.push(entity);
    }

    /// Extend with entities from another collection.
    pub fn extend(&mut self, other: impl IntoIterator<Item = Entity>) {
        self.0.extend(other);
    }

    /// Retain only entities above the given confidence threshold.
    pub fn above_confidence(&self, threshold: f64) -> Self {
        self.0
            .iter()
            .filter(|e| e.confidence >= threshold)
            .cloned()
            .collect()
    }

    /// Retain only entities that were recognised (at least partly) by the given method.
    pub fn by_recognition_method(&self, method: RecognitionMethod) -> Self {
        self.0
            .iter()
            .filter(|e| e.recognition_methods.contains(&method))
            .cloned()
            .collect()
    }

    /// Retain only entities whose content was extracted by the given method.
    pub fn by_extraction_method(&self, method: ExtractionMethod) -> Self {
        self.0
            .iter()
            .filter(|e| e.extraction_methods.contains(&method))
            .cloned()
            .collect()
    }

    /// Retain only entities matching the given category.
    pub fn by_category(&self, category: EntityCategory) -> Self {
        self.0
            .iter()
            .filter(|e| e.category == category)
            .cloned()
            .collect()
    }

    /// Consume and return the inner `Vec<Entity>`.
    pub fn into_inner(self) -> Vec<Entity> {
        self.0
    }
}

impl<'a> IntoIterator for &'a Entities {
    type IntoIter = std::slice::Iter<'a, Entity>;
    type Item = &'a Entity;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl FromIterator<Entity> for Entities {
    fn from_iter<I: IntoIterator<Item = Entity>>(iter: I) -> Self {
        Self(iter.into_iter().collect())
    }
}
