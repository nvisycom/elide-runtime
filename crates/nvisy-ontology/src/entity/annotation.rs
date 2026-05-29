//! User-supplied annotations for pre-identified regions and
//! classification labels.
//!
//! Two kinds of annotation exist with different shapes because they
//! describe different things:
//!
//! - [`Annotation<M>`] (modality-typed): per-region [`Inclusion`] and
//!   [`Exclusion`] markers that target a [`AnnotationTarget<M>`]
//!   (text value, image region, audio segment, …). Both kinds carry
//!   an [`AnnotationStrength`] so the engine knows whether the user
//!   asked for advisory bias or a hard constraint.
//! - [`LabelAnnotation`] (modality-agnostic): document-wide
//!   classification labels that don't reference a per-modality
//!   location. Apply identically to every modality envelope spawned
//!   from the same source.
//!
//! Conversion of [`Assert`] inclusions into [`Entity<M>`]s requires a
//! resolved location, which the engine importer supplies. The data
//! layer provides the [`Annotation::to_inclusion_entity`]
//! conversion; resolution policy and prompt rendering for [`Hint`]
//! variants live at the call site.
//!
//! [`Inclusion`]: AnnotationKind::Inclusion
//! [`Exclusion`]: AnnotationKind::Exclusion
//! [`Assert`]: AnnotationStrength::Assert
//! [`Hint`]: AnnotationStrength::Hint

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
    /// A specific text or data value. Only meaningful when the
    /// modality has a way to resolve the value to a concrete
    /// location — today that means text, where the importer scans
    /// block contents for the value.
    Value(String),
    /// A modality-specific location (text span, image region, audio
    /// segment, tabular cell).
    Location(M),
}

/// How firmly a region annotation should bind detector behavior.
///
/// Inclusion and exclusion both carry a strength so callers can
/// distinguish "please look out for / try to skip this" from
/// "this *is* / *is not* sensitive, no matter what the detector
/// says."
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum AnnotationStrength {
    /// Advisory bias for LLM/VLM detectors. Rendered into the
    /// detector's prompt. Detectors without a prompt surface
    /// (regex, dictionary) ignore the hint; LLM detectors may still
    /// reject it.
    Hint,
    /// Hard constraint enforced by the engine regardless of which
    /// detectors ran. An asserted [`Inclusion`] is materialised as
    /// a synthetic entity at import time; an asserted [`Exclusion`]
    /// drops matching detections post-filter.
    ///
    /// [`Inclusion`]: AnnotationKind::Inclusion
    /// [`Exclusion`]: AnnotationKind::Exclusion
    Assert,
}

/// The kind of region annotation with variant-specific data.
///
/// Document-level labels are not part of this enum — they live on
/// [`LabelAnnotation`] because they don't carry a modality
/// parameter.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    bound(serialize = "M: Serialize", deserialize = "M: DeserializeOwned",)
)]
#[schemars(bound = "M: JsonSchema")]
pub enum AnnotationKind<M: Modality> {
    /// Pre-identified region the user wants treated as sensitive.
    /// The [`strength`](Self::Inclusion::strength) field decides
    /// whether this hints the LLM/VLM detector or asserts a
    /// guaranteed detection.
    Inclusion {
        /// Broad classification of the sensitive data. `None` when
        /// the user wants the region treated as sensitive without
        /// committing to a category — synthesised entities fall
        /// back to [`EntityCategory::Unresolved`].
        #[serde(skip_serializing_if = "Option::is_none")]
        category: Option<EntityCategory>,
        /// Specific entity kind. `None` when the user wants the
        /// region treated as sensitive without committing to a
        /// kind — synthesised entities fall back to
        /// [`EntityKind::Unresolved`].
        #[serde(skip_serializing_if = "Option::is_none")]
        entity_kind: Option<EntityKind>,
        /// What this inclusion targets.
        target: AnnotationTarget<M>,
        /// Whether this is a hint to detectors or a hard assertion.
        strength: AnnotationStrength,
        /// Confidence in the range `[0.0, 1.0]`. Absent annotations
        /// default to full confidence (`1.0`) at conversion time.
        #[serde(skip_serializing_if = "Option::is_none")]
        confidence: Option<Confidence>,
    },
    /// Known-safe region the user wants the engine to skip. The
    /// [`strength`](Self::Exclusion::strength) field decides
    /// whether this hints the LLM/VLM detector or asserts a hard
    /// drop in the post-detection filter.
    Exclusion {
        /// What this exclusion targets.
        target: AnnotationTarget<M>,
        /// Whether this is a hint to detectors or a hard assertion.
        strength: AnnotationStrength,
    },
}

/// A user-provided region annotation on a content region.
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

impl<M: Modality> Annotation<M> {
    /// Convert an [`Assert`]-strength [`Inclusion`] annotation into
    /// an [`Entity<M>`] at the supplied location. Returns `None` for
    /// non-inclusion kinds and for [`Hint`]-strength inclusions
    /// (which the caller should render into a detector prompt
    /// instead).
    ///
    /// Resolution of [`AnnotationTarget::Value`] into a concrete
    /// `M` location is the caller's responsibility — every modality
    /// produces an entity the same way once a location is in hand.
    ///
    /// [`Inclusion`]: AnnotationKind::Inclusion
    /// [`Assert`]: AnnotationStrength::Assert
    /// [`Hint`]: AnnotationStrength::Hint
    pub fn to_inclusion_entity(&self, location: M) -> Option<Entity<M>> {
        let AnnotationKind::Inclusion {
            category,
            entity_kind,
            confidence,
            strength: AnnotationStrength::Assert,
            ..
        } = &self.kind
        else {
            return None;
        };

        let confidence =
            confidence.unwrap_or_else(|| Confidence::new(1.0).expect("1.0 is in [0.0, 1.0]"));
        let entity = Entity::builder()
            .with_category(category.unwrap_or(EntityCategory::Unresolved))
            .with_entity_kind(entity_kind.unwrap_or(EntityKind::Unresolved))
            .with_recognition_methods(vec![RecognitionMethod::annotation(self.name.clone())])
            .with_confidence(confidence)
            .with_location(location)
            .build()
            .expect("required fields provided");
        Some(entity)
    }
}

/// A document-wide classification label.
///
/// Labels are modality-agnostic: a `"medical"` or `"gdpr-request"`
/// tag applies to the source document and propagates to every
/// per-modality [`Document<M>`] spawned from it. Policy rules can
/// gate on labels via conditions.
///
/// [`Document<M>`]: crate::document::Document
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct LabelAnnotation {
    /// Optional human-readable name (provenance / source identifier).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Label value (e.g. `"medical"`, `"gdpr-request"`).
    pub label: String,
}

impl LabelAnnotation {
    /// Build a [`LabelAnnotation`] from a label string.
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            name: None,
            label: label.into(),
        }
    }
}

/// Check whether any [`Assert`]-strength [`Exclusion`] matches
/// `entity`.
///
/// Only [`Assert`] exclusions affect the post-detection filter;
/// [`Hint`] exclusions are advisory and are consumed at prompt
/// build time by LLM/VLM detectors.
///
/// `entity_value` is the detected text value resolved from the
/// document. Pass `None` for entities without a text value (e.g.
/// image-only detections).
///
/// [`Exclusion`]: AnnotationKind::Exclusion
/// [`Assert`]: AnnotationStrength::Assert
/// [`Hint`]: AnnotationStrength::Hint
pub fn is_excluded<M>(
    annotations: &[Annotation<M>],
    entity: &Entity<M>,
    entity_value: Option<&str>,
) -> bool
where
    M: Modality + Overlap,
{
    annotations.iter().any(|ann| {
        let AnnotationKind::Exclusion {
            target,
            strength: AnnotationStrength::Assert,
        } = &ann.kind
        else {
            return false;
        };
        match target {
            AnnotationTarget::Value(value) => entity_value.is_some_and(|v| v == value),
            AnnotationTarget::Location(location) => entity.location.overlaps(location),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modality::Text;
    use crate::primitive::BoundingBox;

    fn inclusion_value(value: &str, strength: AnnotationStrength) -> Annotation<Text> {
        Annotation {
            name: None,
            kind: AnnotationKind::Inclusion {
                category: Some(EntityCategory::PersonalIdentity),
                entity_kind: Some(EntityKind::PersonName),
                target: AnnotationTarget::Value(value.into()),
                strength,
                confidence: None,
            },
        }
    }

    fn inclusion_location(
        start: usize,
        end: usize,
        strength: AnnotationStrength,
    ) -> Annotation<Text> {
        Annotation {
            name: None,
            kind: AnnotationKind::Inclusion {
                category: Some(EntityCategory::PersonalIdentity),
                entity_kind: Some(EntityKind::PersonName),
                target: AnnotationTarget::Location(Text::new(start, end)),
                strength,
                confidence: None,
            },
        }
    }

    fn exclusion_value(value: &str, strength: AnnotationStrength) -> Annotation<Text> {
        Annotation {
            name: None,
            kind: AnnotationKind::Exclusion {
                target: AnnotationTarget::Value(value.into()),
                strength,
            },
        }
    }

    fn exclusion_location(
        start: usize,
        end: usize,
        strength: AnnotationStrength,
    ) -> Annotation<Text> {
        Annotation {
            name: None,
            kind: AnnotationKind::Exclusion {
                target: AnnotationTarget::Location(Text::new(start, end)),
                strength,
            },
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
    fn assert_inclusion_produces_entity_at_location() {
        let ann = inclusion_location(0, 10, AnnotationStrength::Assert);
        let entity = ann.to_inclusion_entity(Text::new(0, 10)).unwrap();
        assert_eq!(entity.location, Text::new(0, 10));
        assert!((entity.confidence.get() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn assert_inclusion_value_uses_supplied_location() {
        let ann = inclusion_value("John Smith", AnnotationStrength::Assert);
        let entity = ann.to_inclusion_entity(Text::new(5, 15)).unwrap();
        assert_eq!(entity.location, Text::new(5, 15));
    }

    #[test]
    fn hint_inclusion_does_not_produce_entity() {
        let ann = inclusion_location(0, 10, AnnotationStrength::Hint);
        assert!(ann.to_inclusion_entity(Text::new(0, 10)).is_none());
    }

    #[test]
    fn unclassified_assert_inclusion_falls_back_to_unresolved() {
        let ann = Annotation {
            name: None,
            kind: AnnotationKind::Inclusion {
                category: None,
                entity_kind: None,
                target: AnnotationTarget::Location(Text::new(0, 10)),
                strength: AnnotationStrength::Assert,
                confidence: None,
            },
        };
        let entity = ann.to_inclusion_entity(Text::new(0, 10)).unwrap();
        assert_eq!(entity.category, EntityCategory::Unresolved);
        assert_eq!(entity.entity_kind, EntityKind::Unresolved);
    }

    #[test]
    fn to_inclusion_entity_returns_none_for_exclusion() {
        let ann = exclusion_value("safe", AnnotationStrength::Assert);
        assert!(ann.to_inclusion_entity(Text::new(0, 4)).is_none());
    }

    #[test]
    fn assert_inclusion_image_uses_supplied_location() {
        use crate::modality::Image;
        let bbox = BoundingBox::new(0.0, 0.0, 10.0, 10.0);
        let ann: Annotation<Image> = Annotation {
            name: Some("face".into()),
            kind: AnnotationKind::Inclusion {
                category: Some(EntityCategory::PersonalIdentity),
                entity_kind: Some(EntityKind::PersonName),
                target: AnnotationTarget::Location(Image::new(bbox)),
                strength: AnnotationStrength::Assert,
                confidence: None,
            },
        };
        let entity = ann.to_inclusion_entity(Image::new(bbox)).unwrap();
        assert_eq!(entity.location.bounding_box, bbox);
    }

    #[test]
    fn assert_exclusion_by_value() {
        let anns = vec![exclusion_value("safe-value", AnnotationStrength::Assert)];
        let entity = test_entity(0, 10);
        assert!(is_excluded(&anns, &entity, Some("safe-value")));
    }

    #[test]
    fn hint_exclusion_is_not_a_post_filter() {
        let anns = vec![exclusion_value("safe-value", AnnotationStrength::Hint)];
        let entity = test_entity(0, 10);
        assert!(!is_excluded(&anns, &entity, Some("safe-value")));
    }

    #[test]
    fn assert_exclusion_by_value_no_match() {
        let anns = vec![exclusion_value("other", AnnotationStrength::Assert)];
        let entity = test_entity(0, 9);
        assert!(!is_excluded(&anns, &entity, Some("sensitive")));
    }

    #[test]
    fn assert_exclusion_by_location_overlap() {
        let anns = vec![exclusion_location(5, 15, AnnotationStrength::Assert)];
        let entity = test_entity(10, 20);
        assert!(is_excluded(&anns, &entity, Some("test")));
    }

    #[test]
    fn assert_exclusion_by_location_no_overlap() {
        let anns = vec![exclusion_location(0, 5, AnnotationStrength::Assert)];
        let entity = test_entity(10, 20);
        assert!(!is_excluded(&anns, &entity, Some("test")));
    }
}
