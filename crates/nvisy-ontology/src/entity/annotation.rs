//! Annotation types for pre-identified regions and classification
//! labels.
//!
//! Annotations are collected as plain `Vec<Annotation<M>>`. Domain
//! helpers ([`is_excluded`], [`inclusion_entities`],
//! [`document_labels`]) are free functions on slices.

use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use super::{Entity, EntityCategory, EntityKind, RecognitionMethod};
use crate::modality::{Modality, Overlap};
use crate::primitive::Confidence;

/// What a region annotation points at: a text value or a
/// modality-specific location.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(
    rename_all = "snake_case",
    bound(serialize = "M: Serialize", deserialize = "M: DeserializeOwned",)
)]
#[schemars(bound = "M: JsonSchema")]
pub enum AnnotationTarget<M: Modality> {
    /// A specific text or data value.
    Value(String),
    /// A modality-specific location (text span, image region, audio
    /// segment).
    Location(M),
}

/// The kind of annotation with variant-specific data.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    bound(serialize = "M: Serialize", deserialize = "M: DeserializeOwned",)
)]
#[schemars(bound = "M: JsonSchema")]
pub enum AnnotationKind<M: Modality> {
    /// Pre-identified sensitive region that should be treated as a
    /// detection.
    Inclusion {
        /// Broad classification of the sensitive data.
        category: EntityCategory,
        /// Specific entity kind.
        entity_kind: EntityKind,
        /// What this inclusion targets.
        target: AnnotationTarget<M>,
        /// Confidence in the range `[0.0, 1.0]`. Absent annotations
        /// default to full confidence (`1.0`) at conversion time.
        #[serde(skip_serializing_if = "Option::is_none")]
        confidence: Option<Confidence>,
    },
    /// Known-safe region that detection should skip.
    Exclusion {
        /// What this exclusion targets.
        target: AnnotationTarget<M>,
    },
    /// Classification label for document-level policy scoping.
    Label {
        /// Label name (e.g. `"medical"`, `"gdpr-request"`).
        label: String,
    },
}

/// A user-provided annotation on a content region.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(
    rename_all = "camelCase",
    bound(serialize = "M: Serialize", deserialize = "M: DeserializeOwned",)
)]
#[schemars(bound = "M: JsonSchema")]
pub struct Annotation<M: Modality> {
    /// Optional human-readable name for this annotation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// What kind of annotation and its variant-specific data.
    #[serde(flatten)]
    pub kind: AnnotationKind<M>,
}

/// All document-level [`Label`] names across `annotations`.
///
/// [`Label`]: AnnotationKind::Label
pub fn document_labels<M: Modality>(annotations: &[Annotation<M>]) -> Vec<&str> {
    annotations
        .iter()
        .filter_map(|a| match &a.kind {
            AnnotationKind::Label { label } => Some(label.as_str()),
            _ => None,
        })
        .collect()
}

/// Check whether any [`Exclusion`] annotation matches `entity`.
///
/// `entity_value` is the detected text value resolved from the
/// document. Pass `None` for entities without a text value (e.g.
/// image-only detections).
///
/// [`Exclusion`]: AnnotationKind::Exclusion
pub fn is_excluded<M>(
    annotations: &[Annotation<M>],
    entity: &Entity<M>,
    entity_value: Option<&str>,
) -> bool
where
    M: Modality + Overlap,
{
    annotations.iter().any(|ann| {
        let AnnotationKind::Exclusion { target } = &ann.kind else {
            return false;
        };
        match target {
            AnnotationTarget::Value(value) => entity_value.is_some_and(|v| v == value),
            AnnotationTarget::Location(location) => entity.location.overlaps(location),
        }
    })
}

/// Convert all [`Inclusion`] annotations into entities.
///
/// Value-only inclusions use `M::default()` as a sentinel location
/// since they don't reference a real document position; the value
/// itself is carried by the recognition method.
///
/// [`Inclusion`]: AnnotationKind::Inclusion
pub fn inclusion_entities<M: Modality + Default>(annotations: &[Annotation<M>]) -> Vec<Entity<M>> {
    let mut out = Vec::new();
    for ann in annotations {
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
                M::default()
            }
            AnnotationTarget::Location(loc) => loc.clone(),
        };

        let confidence =
            confidence.unwrap_or_else(|| Confidence::new(1.0).expect("1.0 is in [0.0, 1.0]"));
        let entity = Entity::builder()
            .with_category(*category)
            .with_entity_kind(*entity_kind)
            .with_recognition_methods(vec![RecognitionMethod::annotation(ann.name.clone())])
            .with_confidence(confidence)
            .with_location(location)
            .build()
            .expect("required fields provided");
        out.push(entity);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modality::Text;

    fn inclusion(value: &str) -> Annotation<Text> {
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

    fn exclusion_value(value: &str) -> Annotation<Text> {
        Annotation {
            name: None,
            kind: AnnotationKind::Exclusion {
                target: AnnotationTarget::Value(value.into()),
            },
        }
    }

    fn exclusion_location(start: usize, end: usize) -> Annotation<Text> {
        Annotation {
            name: None,
            kind: AnnotationKind::Exclusion {
                target: AnnotationTarget::Location(Text::new(start, end)),
            },
        }
    }

    fn label(name: &str) -> Annotation<Text> {
        Annotation {
            name: None,
            kind: AnnotationKind::Label { label: name.into() },
        }
    }

    fn test_entity(start: usize, end: usize) -> Entity<Text> {
        Entity::builder()
            .with_category(EntityCategory::PersonalIdentity)
            .with_entity_kind(EntityKind::PersonName)
            .with_recognition_methods(vec![RecognitionMethod::regex("test")])
            .with_confidence(Confidence::new(0.9).expect("in range"))
            .with_location(Text::new(start, end))
            .build()
            .unwrap()
    }

    #[test]
    fn inclusion_entities_creates_entities() {
        let anns = vec![inclusion("John Smith"), inclusion("jane@example.com")];
        let entities = inclusion_entities(&anns);
        assert_eq!(entities.len(), 2);
        assert_eq!(entities[0].location.start, 0);
        assert_eq!(entities[0].location.end, 0);
        assert!((entities[0].confidence.get() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn inclusion_entities_skips_empty_value() {
        let anns = vec![inclusion("")];
        let entities = inclusion_entities(&anns);
        assert!(entities.is_empty());
    }

    #[test]
    fn exclusion_by_value() {
        let anns = vec![exclusion_value("safe-value")];
        let entity = test_entity(0, 10);
        assert!(is_excluded(&anns, &entity, Some("safe-value")));
    }

    #[test]
    fn exclusion_by_value_no_match() {
        let anns = vec![exclusion_value("other")];
        let entity = test_entity(0, 9);
        assert!(!is_excluded(&anns, &entity, Some("sensitive")));
    }

    #[test]
    fn exclusion_by_location_overlap() {
        let anns = vec![exclusion_location(5, 15)];
        let entity = test_entity(10, 20);
        assert!(is_excluded(&anns, &entity, Some("test")));
    }

    #[test]
    fn exclusion_by_location_no_overlap() {
        let anns = vec![exclusion_location(0, 5)];
        let entity = test_entity(10, 20);
        assert!(!is_excluded(&anns, &entity, Some("test")));
    }

    #[test]
    fn document_labels_extracts_label_annotations() {
        let anns = vec![
            label("medical"),
            inclusion("test"),
            label("gdpr"),
            exclusion_value("safe"),
        ];
        let labels = document_labels(&anns);
        assert_eq!(labels, vec!["medical", "gdpr"]);
    }
}
