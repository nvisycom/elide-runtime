//! Annotation types for pre-identified regions and classification labels.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{Entities, Entity, EntityCategory, EntityKind, Location, RecognitionMethod};
use crate::entity::Overlap;

/// What a region annotation points at: a text value or a spatial/temporal location.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AnnotationTarget {
    /// A specific text or data value.
    Value(String),
    /// A modality-specific location (text span, image region, audio segment).
    Location(Location),
}

/// The kind of annotation with variant-specific data.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AnnotationKind {
    /// Pre-identified sensitive region that should be treated as a detection.
    Inclusion {
        /// Broad classification of the sensitive data.
        category: EntityCategory,
        /// Specific entity kind.
        entity_kind: EntityKind,
        /// What this inclusion targets.
        target: AnnotationTarget,
    },
    /// Known-safe region that detection should skip.
    Exclusion {
        /// What this exclusion targets.
        target: AnnotationTarget,
    },
    /// Classification label for document-level policy scoping.
    Label {
        /// Label name (e.g. `"medical"`, `"gdpr-request"`).
        label: String,
    },
}

/// A user-provided annotation on a content region.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Annotation {
    /// What kind of annotation and its variant-specific data.
    #[serde(flatten)]
    pub kind: AnnotationKind,
    /// Confidence of the annotation in the range `[0.0, 1.0]`.
    /// Defaults to 1.0 for inclusions when not specified.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f64>,
}

/// A collection of [`Annotation`]s.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Annotations(Vec<Annotation>);

impl Annotations {
    /// Create an empty collection.
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of annotations.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns `true` if the collection is empty.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Iterate over all annotations.
    pub fn iter(&self) -> std::slice::Iter<'_, Annotation> {
        self.0.iter()
    }

    /// Add an annotation.
    pub fn push(&mut self, annotation: Annotation) {
        self.0.push(annotation);
    }

    /// All document-level label names.
    pub fn document_labels(&self) -> Vec<&str> {
        self.0
            .iter()
            .filter_map(|a| match &a.kind {
                AnnotationKind::Label { label } => Some(label.as_str()),
                _ => None,
            })
            .collect()
    }

    /// Check whether the given entity falls within any exclusion annotation.
    ///
    /// An entity is excluded if an exclusion targets a matching value
    /// (exact) or an overlapping location (any modality).
    pub fn is_excluded(&self, entity: &Entity) -> bool {
        for ann in &self.0 {
            let AnnotationKind::Exclusion { target } = &ann.kind else {
                continue;
            };
            match target {
                AnnotationTarget::Value(value) if *value == entity.value => return true,
                AnnotationTarget::Location(location) => {
                    if entity
                        .location
                        .as_ref()
                        .is_some_and(|loc| loc.overlaps(location))
                    {
                        return true;
                    }
                }
                _ => {}
            }
        }
        false
    }

    /// Convert inclusion annotations into entities and add them to the collection.
    pub fn apply_inclusions(&self, entities: &mut Entities) {
        for ann in &self.0 {
            let AnnotationKind::Inclusion {
                category,
                entity_kind,
                target,
            } = &ann.kind
            else {
                continue;
            };

            let (value, location) = match target {
                AnnotationTarget::Value(value) => {
                    if value.is_empty() {
                        continue;
                    }
                    (value.clone(), None)
                }
                AnnotationTarget::Location(location) => {
                    (String::new(), Some(location.clone()))
                }
            };

            let confidence = ann.confidence.unwrap_or(1.0);
            let mut builder = Entity::builder()
                .with_category(*category)
                .with_entity_kind(*entity_kind)
                .with_value(value)
                .with_recognition_methods(vec![RecognitionMethod::annotation(None)])
                .with_confidence(confidence);
            if let Some(loc) = location {
                builder = builder.with_location(loc);
            }
            entities.push(builder.build().expect("required fields provided"));
        }
    }
}

impl From<Vec<Annotation>> for Annotations {
    fn from(v: Vec<Annotation>) -> Self {
        Self(v)
    }
}
