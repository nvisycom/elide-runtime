//! Sensitive-data entity types and detection metadata.
//!
//! [`Entity<M>`] represents a single occurrence of sensitive data
//! detected within a document. Detected entities are accumulated on
//! the run-scoped [`Audit<M>`] as [`EntityRecord<M>`]s — each record
//! bundles the entity with the optional [`AuditEntry<M>`] produced
//! for it during redaction.
//!
//! Each entity carries a chronological [`TrailStep`] list explaining
//! how it reached its final confidence: the base recognizer firing,
//! any refinement / verification / fusion / calibration steps, and
//! the score before and after each. This single trail replaces the
//! prior parallel pair of recognition + refinement method lists.
//!
//! [`Audit<M>`]: https://docs.rs/nvisy-engine/latest/nvisy_engine/provenance/struct.Audit.html
//! [`EntityRecord<M>`]: https://docs.rs/nvisy-engine/latest/nvisy_engine/provenance/struct.EntityRecord.html
//! [`AuditEntry<M>`]: https://docs.rs/nvisy-engine/latest/nvisy_engine/provenance/struct.AuditEntry.html

mod annotation;
mod category;
mod kind;
mod method;
mod source;

use derive_builder::Builder;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub use self::annotation::{
    Annotation, AnnotationKind, AnnotationStrength, LabelAnnotation, is_excluded,
};
pub use self::category::EntityCategory;
pub use self::kind::EntityKind;
pub use self::method::{
    AnnotationProvenance, ModelProvenance, PatternProvenance, TrailProvenance, TrailStep,
    TrailStepKind,
};
pub use self::source::ContentSource;
use crate::modality::Modality;
#[cfg(any(test, feature = "test-utils"))]
use crate::modality::{Text, TextLocation};
use crate::primitive::{Confidence, LanguageTag};

/// A detected sensitive data occurrence within a document.
///
/// The category for an entity is derived from its [`entity_kind`] via
/// [`EntityKind::category`]; it is not stored separately. The trail
/// of score-affecting steps lives on [`trail`].
///
/// [`entity_kind`]: Self::entity_kind
/// [`trail`]: Self::trail
#[derive(Debug, Clone, PartialEq, Builder)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[builder(
    name = "EntityBuilder",
    pattern = "owned",
    setter(into, strip_option, prefix = "with")
)]
#[serde(rename_all = "camelCase")]
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
    /// Specific entity kind. The broad [`EntityCategory`] is derived
    /// via [`Entity::category`].
    pub entity_kind: EntityKind,
    /// Modality-specific location of the entity within the document.
    pub location: M::Location,
    /// Detection confidence score in the range `[0.0, 1.0]`. Equals
    /// the `adjusted` score on the final step in `trail`.
    pub confidence: Confidence,
    /// Chronological trail of score-affecting steps: the base
    /// recognition, any refinement / verification / fusion /
    /// calibration adjustments, each tagged with `original` /
    /// `adjusted` score and a free-text reason.
    #[builder(default)]
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub trail: Vec<TrailStep>,
    /// BCP-47 language tag for the span the entity covers, when the
    /// recognizer has one to assign. Locale-aware anonymizers (the
    /// fake-data generator in `nvisy-fake`, format-preserving
    /// templates) read this to pick a locale. `None` falls back to
    /// the document-level language metadata.
    #[builder(default)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<String>")]
    pub language: Option<LanguageTag>,
}

impl<M: Modality> Entity<M> {
    /// Create a new [`EntityBuilder`].
    pub fn builder() -> EntityBuilder<M> {
        EntityBuilder::default()
    }

    /// Derived broad classification — `self.entity_kind.category()`.
    #[must_use]
    pub fn category(&self) -> EntityCategory {
        self.entity_kind.category()
    }

    /// Original recognition score, before any post-recognition
    /// adjustments. Reads from the first step's `original` (or
    /// `adjusted` if it had none), returning `None` only if the
    /// trail is empty.
    #[must_use]
    pub fn original_score(&self) -> Option<Confidence> {
        self.trail.first().map(|s| s.original.unwrap_or(s.adjusted))
    }

    /// Final confidence — same as [`Self::confidence`], exposed as a
    /// method for symmetry with [`Self::original_score`].
    #[must_use]
    pub fn final_score(&self) -> Confidence {
        self.confidence
    }

    /// Names of the recognizers that produced this entity (one or
    /// more, in dispatch order). Reads the [`TrailStepKind::Recognition`]
    /// steps' `source` field.
    pub fn recognizers(&self) -> impl Iterator<Item = &str> {
        self.trail
            .iter()
            .filter(|s| s.kind == TrailStepKind::Recognition)
            .map(|s| s.source.as_str())
    }
}

#[cfg(any(test, feature = "test-utils"))]
impl Entity<Text> {
    /// Create a pre-filled [`EntityBuilder`] for tests.
    ///
    /// Defaults: `PersonName` / a synthetic `pattern` recognition
    /// step / confidence `0.9` / text location at `start..end`.
    pub fn test_builder(start: usize, end: usize) -> EntityBuilder<Text> {
        let conf = Confidence::clamped(0.9);
        Entity::builder()
            .with_entity_kind(EntityKind::PersonName)
            .with_trail(vec![TrailStep::recognition(
                "pattern",
                conf,
                TrailProvenance::Pattern(PatternProvenance::Regex {
                    name: "test".to_owned(),
                    regex: None,
                    validator: None,
                    contextual: false,
                }),
                "test fixture",
            )])
            .with_confidence(conf)
            .with_location(TextLocation::new(start, end))
    }
}

#[cfg(any(test, feature = "test-utils"))]
impl<M: Modality> EntityBuilder<M> {
    /// Build the entity, panicking on missing fields.
    pub fn test_build(self) -> Entity<M> {
        self.build().expect("test entity builder: missing fields")
    }
}
