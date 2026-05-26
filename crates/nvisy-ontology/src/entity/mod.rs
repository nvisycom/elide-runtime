//! Sensitive-data entity types and detection metadata.
//!
//! [`Entity<M>`] represents a single occurrence of sensitive data
//! detected within a document. Collections are plain `Vec<Entity<M>>`
//! attached per-block to [`Block::entities`].
//!
//! [`Block::entities`]: crate::document::Block::entities

mod annotation;
mod category;
mod kind;
mod method;
mod sensitivity;
mod source;

use derive_builder::Builder;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub use self::annotation::{
    Annotation, AnnotationKind, AnnotationTarget, document_labels, inclusion_entities,
};
pub use self::category::EntityCategory;
pub use self::kind::EntityKind;
pub use self::method::{
    AnnotationProvenance, CrossReferenceProvenance, ExtractionMethod, ModelKind, ModelProvenance,
    PatternKind, PatternProvenance, RecognitionMethod, RecognitionMethodKind, RefinementMethod,
};
pub use self::sensitivity::EntitySensitivity;
pub use self::source::ContentSource;
use crate::modality::Modality;
#[cfg(any(test, feature = "test-utils"))]
use crate::modality::Text;
use crate::primitive::{Confidence, LanguageTag};

/// A detected sensitive data occurrence within a document.
#[derive(Debug, Clone, PartialEq, Builder)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[builder(
    name = "EntityBuilder",
    pattern = "owned",
    setter(into, strip_option, prefix = "with")
)]
#[serde(
    rename_all = "camelCase",
    bound(
        serialize = "M: Serialize",
        deserialize = "M: serde::de::DeserializeOwned",
    )
)]
#[schemars(bound = "M: JsonSchema")]
pub struct Entity<M: Modality> {
    /// Unique identifier for this entity (UUIDv7).
    #[builder(default = "Uuid::now_v7()")]
    pub id: Uuid,
    /// NER/LLM-assigned coreference identifier, stable across
    /// coreferent mentions within one detection call. Two entities
    /// sharing `entity_id` refer to the same real-world entity.
    #[builder(default, setter(into = false))]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entity_id: Option<String>,
    /// Broad classification of the sensitive data.
    pub category: EntityCategory,
    /// Specific entity kind.
    pub entity_kind: EntityKind,
    /// How content was extracted from its source modality, ordered by
    /// application time.
    #[builder(default)]
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extraction_methods: Vec<ExtractionMethod>,
    /// Techniques used to identify this entity, ordered by
    /// application time.
    pub recognition_methods: Vec<RecognitionMethod>,
    /// Post-detection refinements applied to this entity, ordered by
    /// application time.
    #[builder(default)]
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub refinement_methods: Vec<RefinementMethod>,
    /// Detection confidence score in the range `[0.0, 1.0]`.
    pub confidence: Confidence,
    /// Modality-specific location of the entity within the document.
    pub location: M,
    /// BCP-47 language tag of the detected content.
    #[builder(default, setter(into = false))]
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<String>")]
    pub language: Option<LanguageTag>,
    /// Sensitivity classification of this entity.
    #[builder(default, setter(into = false))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sensitivity: Option<EntitySensitivity>,
}

impl<M: Modality> Entity<M> {
    /// Create a new [`EntityBuilder`].
    pub fn builder() -> EntityBuilder<M> {
        EntityBuilder::default()
    }
}

#[cfg(any(test, feature = "test-utils"))]
impl Entity<Text> {
    /// Create a pre-filled [`EntityBuilder`] for tests.
    ///
    /// Defaults: `PersonalIdentity` / `PersonName` / `regex("test")` /
    /// confidence `0.9` / text location at `start..end`.
    pub fn test_builder(start: usize, end: usize) -> EntityBuilder<Text> {
        Entity::builder()
            .with_category(EntityCategory::PersonalIdentity)
            .with_entity_kind(EntityKind::PersonName)
            .with_recognition_methods(vec![RecognitionMethod::regex("test")])
            .with_confidence(Confidence::clamped(0.9))
            .with_location(Text::new(start, end))
    }
}

#[cfg(any(test, feature = "test-utils"))]
impl<M: Modality> EntityBuilder<M> {
    /// Build the entity, panicking on missing fields.
    pub fn test_build(self) -> Entity<M> {
        self.build().expect("test entity builder: missing fields")
    }
}
