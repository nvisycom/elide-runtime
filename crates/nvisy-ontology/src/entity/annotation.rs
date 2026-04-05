//! Annotation types for pre-identified regions and classification labels.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{
    Entities, Entity, EntityCategory, EntityKind, Location, RecognitionMethod, TextLocation,
};
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
        /// Confidence in the range `[0.0, 1.0]` (default 1.0).
        #[serde(skip_serializing_if = "Option::is_none")]
        confidence: Option<f64>,
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
    /// Optional human-readable name for this annotation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// What kind of annotation and its variant-specific data.
    #[serde(flatten)]
    pub kind: AnnotationKind,
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
    /// An entity is excluded if an exclusion targets an overlapping
    /// location or a matching text value. The `entity_value` parameter
    /// is the text at the entity's location, extracted from the
    /// document by the caller (since the annotation layer has no
    /// document access).
    pub fn is_excluded(&self, entity: &Entity) -> bool {
        self.0.iter().any(|ann| {
            let AnnotationKind::Exclusion { target } = &ann.kind else {
                return false;
            };
            match target {
                AnnotationTarget::Value(value) => entity.text_value().is_some_and(|v| v == value),
                AnnotationTarget::Location(location) => entity.location.overlaps(location),
            }
        })
    }

    /// Convert inclusion annotations into entities and add them to the collection.
    pub fn apply_inclusions(&self, entities: &mut Entities) {
        for ann in &self.0 {
            let AnnotationKind::Inclusion {
                category,
                entity_kind,
                target,
                confidence,
            } = &ann.kind
            else {
                continue;
            };

            let location = match target {
                AnnotationTarget::Value(v) => {
                    if v.is_empty() {
                        continue;
                    }
                    Location::Text(
                        TextLocation::builder()
                            .with_text(v.as_str())
                            .with_start_offset(0usize)
                            .with_end_offset(v.len())
                            .build()
                            .expect("required fields provided"),
                    )
                }
                AnnotationTarget::Location(loc) => loc.clone(),
            };

            let entity = Entity::builder()
                .with_category(*category)
                .with_entity_kind(*entity_kind)
                .with_recognition_methods(vec![RecognitionMethod::annotation(ann.name.clone())])
                .with_confidence(confidence.unwrap_or(1.0))
                .with_location(location)
                .build()
                .expect("required fields provided");
            entities.push(entity);
        }
    }
}

impl From<Vec<Annotation>> for Annotations {
    fn from(v: Vec<Annotation>) -> Self {
        Self(v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::TextLocation;

    fn inclusion(value: &str) -> Annotation {
        Annotation {
            name: None,
            kind: AnnotationKind::Inclusion {
                category: EntityCategory::PersonalIdentity,
                entity_kind: EntityKind::PersonName,
                target: AnnotationTarget::Value(value.into()),
                confidence: None,
            },
        }
    }

    fn exclusion_value(value: &str) -> Annotation {
        Annotation {
            name: None,
            kind: AnnotationKind::Exclusion {
                target: AnnotationTarget::Value(value.into()),
            },
        }
    }

    fn exclusion_location(start: usize, end: usize) -> Annotation {
        Annotation {
            name: None,
            kind: AnnotationKind::Exclusion {
                target: AnnotationTarget::Location(Location::from(
                    TextLocation::builder()
                        .with_text("")
                        .with_start_offset(start)
                        .with_end_offset(end)
                        .build()
                        .unwrap(),
                )),
            },
        }
    }

    fn label(name: &str) -> Annotation {
        Annotation {
            name: None,
            kind: AnnotationKind::Label { label: name.into() },
        }
    }

    fn test_entity(value: &str, start: usize, end: usize) -> Entity {
        Entity::builder()
            .with_category(EntityCategory::PersonalIdentity)
            .with_entity_kind(EntityKind::PersonName)
            .with_recognition_methods(vec![RecognitionMethod::regex("test")])
            .with_confidence(0.9)
            .with_location(Location::from(
                TextLocation::builder()
                    .with_text(value)
                    .with_start_offset(start)
                    .with_end_offset(end)
                    .build()
                    .unwrap(),
            ))
            .build()
            .unwrap()
    }

    #[test]
    fn apply_inclusions_creates_entities() {
        let annotations =
            Annotations::from(vec![inclusion("John Smith"), inclusion("jane@example.com")]);
        let mut entities = Entities::new();
        annotations.apply_inclusions(&mut entities);
        assert_eq!(entities.len(), 2);
        // Inclusion entities have TextLocation with start=0, end=value.len().
        assert_eq!(entities[0].text_value(), Some("John Smith"));
        assert_eq!(entities[1].text_value(), Some("jane@example.com"));
        assert!((entities[0].confidence - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn apply_inclusions_skips_empty_value() {
        let annotations = Annotations::from(vec![inclusion("")]);
        let mut entities = Entities::new();
        annotations.apply_inclusions(&mut entities);
        assert!(entities.is_empty());
    }

    #[test]
    fn apply_inclusions_uses_annotation_confidence() {
        let ann = Annotation {
            name: None,
            kind: AnnotationKind::Inclusion {
                category: EntityCategory::PersonalIdentity,
                entity_kind: EntityKind::PersonName,
                target: AnnotationTarget::Value("test".into()),
                confidence: Some(0.7),
            },
        };
        let mut entities = Entities::new();
        Annotations::from(vec![ann]).apply_inclusions(&mut entities);
        assert!((entities[0].confidence - 0.7).abs() < f64::EPSILON);
    }

    #[test]
    fn apply_inclusions_records_annotation_name() {
        let ann = Annotation {
            name: Some("hr-list".into()),
            kind: AnnotationKind::Inclusion {
                category: EntityCategory::PersonalIdentity,
                entity_kind: EntityKind::PersonName,
                target: AnnotationTarget::Value("test".into()),
                confidence: None,
            },
        };
        let mut entities = Entities::new();
        Annotations::from(vec![ann]).apply_inclusions(&mut entities);
        assert_eq!(
            entities[0].recognition_methods[0],
            RecognitionMethod::annotation(Some("hr-list".into()))
        );
    }

    #[test]
    fn exclusion_by_value() {
        let annotations = Annotations::from(vec![exclusion_value("safe-value")]);
        let entity = test_entity("safe-value", 0, 10);
        assert!(annotations.is_excluded(&entity));
    }

    #[test]
    fn exclusion_by_value_no_match() {
        let annotations = Annotations::from(vec![exclusion_value("other")]);
        let entity = test_entity("sensitive", 0, 9);
        assert!(!annotations.is_excluded(&entity));
    }

    #[test]
    fn exclusion_by_location_overlap() {
        let annotations = Annotations::from(vec![exclusion_location(5, 15)]);
        let entity = test_entity("test", 10, 20);
        assert!(annotations.is_excluded(&entity));
    }

    #[test]
    fn exclusion_by_location_no_overlap() {
        let annotations = Annotations::from(vec![exclusion_location(0, 5)]);
        let entity = test_entity("test", 10, 20);
        assert!(!annotations.is_excluded(&entity));
    }

    #[test]
    fn document_labels_extracts_label_annotations() {
        let annotations = Annotations::from(vec![
            label("medical"),
            inclusion("test"),
            label("gdpr"),
            exclusion_value("safe"),
        ]);
        let labels = annotations.document_labels();
        assert_eq!(labels, vec!["medical", "gdpr"]);
    }

    #[test]
    fn empty_annotations_exclude_nothing() {
        let annotations = Annotations::new();
        let entity = test_entity("anything", 0, 8);
        assert!(!annotations.is_excluded(&entity));
    }
}
