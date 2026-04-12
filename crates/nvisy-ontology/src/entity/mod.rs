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
pub use self::location::{
    AudioLocation, AudioLocationBuilder, ImageLocation, ImageLocationBuilder, Location, Overlap,
    TabularLocation, TabularLocationBuilder, TextLocation, TextLocationBuilder,
};
pub use self::method::{
    AnnotationProvenance, ExtractionMethod, ModelKind, ModelProvenance, PatternProvenance,
    RecognitionMethod, RecognitionMethodKind, RefinementMethod,
};
pub use self::sensitivity::EntitySensitivity;
pub use self::source::ContentSource;

/// A detected sensitive data occurrence within a document.
#[derive(Debug, Clone, PartialEq, Builder)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[builder(
    name = "EntityBuilder",
    pattern = "owned",
    setter(into, strip_option, prefix = "with")
)]
#[serde(rename_all = "camelCase")]
pub struct Entity {
    /// Unique identifier for this entity (UUIDv7).
    #[builder(default = "Uuid::now_v7()")]
    pub id: Uuid,
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
    pub confidence: f64,
    /// Modality-specific location of the entity within the document.
    pub location: Location,
    /// BCP-47 language tag of the detected content.
    #[builder(default, setter(into = false))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    /// Sensitivity classification of this entity.
    #[builder(default, setter(into = false))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sensitivity: Option<EntitySensitivity>,
}

impl Entity {
    /// Create a new [`EntityBuilder`].
    pub fn builder() -> EntityBuilder {
        EntityBuilder::default()
    }

    /// Create a pre-filled [`EntityBuilder`] for tests.
    ///
    /// Defaults: `PersonalIdentity` / `PersonName` / `regex("test")` /
    /// confidence `0.9` / text location at `start..end`.
    #[cfg(any(test, feature = "test-utils"))]
    pub fn test_builder(start: usize, end: usize) -> EntityBuilder {
        Entity::builder()
            .with_category(EntityCategory::PersonalIdentity)
            .with_entity_kind(EntityKind::PersonName)
            .with_recognition_methods(vec![RecognitionMethod::regex("test")])
            .with_confidence(0.9)
            .with_location(Location::from(
                TextLocation::builder()
                    .with_start_offset(start)
                    .with_end_offset(end)
                    .build()
                    .expect("required fields provided"),
            ))
    }

    /// The detected text value, if available for this entity's location.
    ///
    /// Not serialized in API responses. Delegates to
    /// [`Location::text_value`].
    pub fn text_value(&self) -> Option<&str> {
        self.location.text_value()
    }
}

#[cfg(any(test, feature = "test-utils"))]
impl EntityBuilder {
    /// Build the entity, panicking on missing fields.
    ///
    /// Shorthand for `.build().expect(...)` in tests.
    pub fn test_build(self) -> Entity {
        self.build().expect("test entity builder: missing fields")
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

    /// Returns `true` if the collection is empty.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Number of entities in the collection.
    pub fn len(&self) -> usize {
        self.0.len()
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

#[cfg(test)]
mod tests {
    use super::*;

    fn entity(confidence: f64) -> Entity {
        Entity::builder()
            .with_category(EntityCategory::PersonalIdentity)
            .with_entity_kind(EntityKind::PersonName)
            .with_recognition_methods(vec![RecognitionMethod::regex("test")])
            .with_confidence(confidence)
            .with_location(Location::from(
                TextLocation::builder()
                    .with_start_offset(0usize)
                    .with_end_offset(4usize)
                    .build()
                    .unwrap(),
            ))
            .build()
            .unwrap()
    }

    #[test]
    fn above_confidence_filters() {
        let entities: Entities = vec![entity(0.9), entity(0.3), entity(0.7)].into();
        let filtered = entities.above_confidence(0.5);
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn above_confidence_empty() {
        let entities = Entities::new();
        assert!(entities.above_confidence(0.5).is_empty());
    }

    #[test]
    fn text_value_from_location() {
        let mut e = entity(0.9);
        if let Location::Text(ref mut loc) = e.location {
            loc.text = "hello".to_string();
        }
        assert_eq!(e.text_value(), Some("hello"));
    }

    #[test]
    fn text_value_empty_string_returns_none() {
        let e = entity(0.9);
        assert_eq!(e.text_value(), None);
    }

    #[test]
    fn entities_from_vec() {
        let v = vec![entity(0.5), entity(0.8)];
        let entities = Entities::from(v);
        assert_eq!(entities.len(), 2);
    }

    #[test]
    fn entities_collect() {
        let entities: Entities = (0..3).map(|i| entity(i as f64 * 0.3)).collect();
        assert_eq!(entities.len(), 3);
    }
}
