//! Sensitive-data entity types and detection metadata.
//!
//! An [`Entity`] represents a single occurrence of sensitive data
//! detected within a document. [`Entities`] is the canonical
//! collection type used across the pipeline.
//!
//! Both types are generic over `M: Modality`, the per-modality
//! coordinate type ([`Text`], [`Image`], [`Audio`], [`Tabular`]).
//! Recognizers wired for one modality cannot be passed entities from
//! another at compile time.
//!
//! [`Modality`]: crate::modality::Modality
//! [`Text`]: crate::modality::Text
//! [`Image`]: crate::modality::Image
//! [`Audio`]: crate::modality::Audio
//! [`Tabular`]: crate::modality::Tabular

mod annotation;
mod category;
mod kind;
mod method;
mod sensitivity;
mod source;

use derive_builder::Builder;
use derive_more::{Deref, DerefMut, From, IntoIterator};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub use self::annotation::{Annotation, AnnotationKind, AnnotationTarget, Annotations};
pub use self::category::EntityCategory;
pub use self::kind::EntityKind;
pub use self::method::{
    AnnotationProvenance, CrossReferenceProvenance, ExtractionMethod, ModelKind, ModelProvenance,
    PatternKind, PatternProvenance, RecognitionMethod, RecognitionMethodKind, RefinementMethod,
};
pub use self::sensitivity::EntitySensitivity;
pub use self::source::ContentSource;
use crate::modality::{AnyModality, Modality};
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
#[serde(rename_all = "camelCase", bound(
    serialize = "M: Serialize",
    deserialize = "M: serde::de::DeserializeOwned",
))]
#[schemars(bound = "M: JsonSchema")]
pub struct Entity<M: Modality> {
    /// Unique identifier for this entity (UUIDv7).
    #[builder(default = "Uuid::now_v7()")]
    pub id: Uuid,
    /// NER/LLM-assigned coreference identifier, stable across coreferent
    /// mentions within one detection call. Two entities sharing
    /// `entity_id` refer to the same real-world entity (different
    /// surface mentions).
    #[builder(default, setter(into = false))]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entity_id: Option<String>,
    /// Broad classification of the sensitive data.
    pub category: EntityCategory,
    /// Specific entity kind (e.g. `GovernmentId`, `EmailAddress`, `PaymentCard`).
    pub entity_kind: EntityKind,
    /// How content was extracted from its source modality, ordered by application time.
    #[builder(default)]
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extraction_methods: Vec<ExtractionMethod>,
    /// Techniques used to identify this entity, ordered by application time.
    pub recognition_methods: Vec<RecognitionMethod>,
    /// Post-detection refinements applied to this entity, ordered by application time.
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

impl<M> Entity<M>
where
    M: Modality + Into<AnyModality>,
{
    /// Lift this entity into a type-erased [`Entity<AnyModality>`] by
    /// converting its location through [`Into::into`].
    ///
    /// The boundary where per-modality typing is dropped — call this
    /// at recognizer-to-audit handoff. Every other field is preserved
    /// verbatim.
    pub fn erase(self) -> Entity<AnyModality> {
        Entity {
            id: self.id,
            entity_id: self.entity_id,
            category: self.category,
            entity_kind: self.entity_kind,
            extraction_methods: self.extraction_methods,
            recognition_methods: self.recognition_methods,
            refinement_methods: self.refinement_methods,
            confidence: self.confidence,
            location: self.location.into(),
            language: self.language,
            sensitivity: self.sensitivity,
        }
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
    ///
    /// Shorthand for `.build().expect(...)` in tests.
    pub fn test_build(self) -> Entity<M> {
        self.build().expect("test entity builder: missing fields")
    }
}

/// A collection of detected entities for one modality.
///
/// Wraps `Vec<Entity<M>>` with transparent `Deref`/`DerefMut` access
/// and domain-specific filtering helpers.
#[derive(Debug, Clone, PartialEq)]
#[derive(Deref, DerefMut, From, IntoIterator)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(transparent, bound(
    serialize = "M: Serialize",
    deserialize = "M: serde::de::DeserializeOwned",
))]
#[schemars(bound = "M: JsonSchema")]
pub struct Entities<M: Modality>(#[into_iterator(owned, ref, ref_mut)] pub Vec<Entity<M>>);

impl<M: Modality> Default for Entities<M> {
    fn default() -> Self {
        Self(Vec::new())
    }
}

impl<M: Modality> Entities<M> {
    /// Create an empty collection.
    pub fn new() -> Self {
        Self(Vec::new())
    }
}

impl<M: Modality> FromIterator<Entity<M>> for Entities<M> {
    fn from_iter<I: IntoIterator<Item = Entity<M>>>(iter: I) -> Self {
        Self(iter.into_iter().collect())
    }
}

