//! User-supplied annotations for pre-identified regions and
//! classification labels.
//!
//! Two kinds of annotation exist with different shapes because they
//! describe different things:
//!
//! - [`Annotation<M>`] (modality-typed): per-region [`Inclusion`] and
//!   [`Exclusion`] markers that target a concrete `M` location
//!   (text span, image region, audio segment, tabular cell). Both
//!   kinds carry an [`AnnotationStrength`] so the engine knows
//!   whether the user asked for advisory bias or a hard constraint.
//! - [`LabelAnnotation`] (modality-agnostic): document-wide
//!   classification labels that don't reference a per-modality
//!   location. Apply identically to every modality envelope spawned
//!   from the same source.
//!
//! Value-style "treat this exact string as sensitive everywhere it
//! appears" annotations are intentionally not part of this surface;
//! that role belongs to the dictionary / deny-list pattern path,
//! which is configured per run rather than per upload.
//!
//! [`Inclusion`]: AnnotationKind::Inclusion
//! [`Exclusion`]: AnnotationKind::Exclusion

use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use super::{Entity, EntityCategory, EntityKind, RecognitionMethod};
use crate::modality::{Modality, Overlap};
use crate::primitive::Confidence;

/// How firmly a region annotation should bind detector behavior.
///
/// Inclusion and exclusion both carry a strength so callers can
/// distinguish "please look out for / try to skip this" from
/// "this *is* / *is not* sensitive, no matter what the detector
/// says."
///
/// Confidence lives on [`Hint`] only because asserting
/// a thing is, by definition, certain — synthesised entities from
/// asserted inclusions always materialise at full confidence
/// (`1.0`).
///
/// [`Hint`]: Self::Hint
#[derive(Debug, Clone, Copy, PartialEq)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[non_exhaustive]
pub enum AnnotationStrength {
    /// Advisory bias for LLM/VLM detectors. Rendered into the
    /// detector's prompt. Detectors without a prompt surface
    /// (regex, dictionary) ignore the hint; LLM detectors may still
    /// reject it.
    ///
    /// `confidence` is forwarded to the prompt and recorded on
    /// detected entities if the LLM honours the hint. `None`
    /// defaults to `1.0` when materialised.
    Hint {
        #[serde(skip_serializing_if = "Option::is_none")]
        confidence: Option<Confidence>,
    },
    /// Hard constraint enforced by the engine regardless of which
    /// detectors ran. An asserted [`Inclusion`] is materialised as
    /// a synthetic entity at import time at full confidence (`1.0`);
    /// an asserted [`Exclusion`] drops matching detections
    /// post-filter.
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
        /// Modality-specific location this inclusion targets.
        target: M,
        /// Whether this is an advisory [`Hint`] (LLM may reject) or
        /// a hard [`Assert`] (engine enforces regardless of
        /// detectors). Lives on `Inclusion` only because exclusions
        /// are always assertions — there's no meaningful "maybe
        /// safe" mode.
        ///
        /// [`Hint`]: AnnotationStrength::Hint
        /// [`Assert`]: AnnotationStrength::Assert
        strength: AnnotationStrength,
    },
    /// Known-safe region the user wants the engine to skip.
    /// Exclusions are always treated as assertions — there is no
    /// "hint" exclusion variant because letting the LLM second-guess
    /// a user's safety claim would defeat the point.
    Exclusion {
        /// Modality-specific location this exclusion targets.
        target: M,
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
    /// an [`Entity<M>`]. Returns `None` for non-inclusion kinds and
    /// for [`Hint`]-strength inclusions (which the caller should
    /// render into a detector prompt instead).
    ///
    /// [`Inclusion`]: AnnotationKind::Inclusion
    /// [`Assert`]: AnnotationStrength::Assert
    /// [`Hint`]: AnnotationStrength::Hint
    pub fn to_inclusion_entity(&self) -> Option<Entity<M>> {
        let AnnotationKind::Inclusion {
            category,
            entity_kind,
            target,
            strength: AnnotationStrength::Assert,
        } = &self.kind
        else {
            return None;
        };

        let entity = Entity::builder()
            .with_category(category.unwrap_or(EntityCategory::Unresolved))
            .with_entity_kind(entity_kind.unwrap_or(EntityKind::Unresolved))
            .with_recognition_methods(vec![RecognitionMethod::annotation(self.name.clone())])
            .with_confidence(Confidence::MAX)
            .with_location(target.clone())
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

/// Check whether any [`Exclusion`] overlaps the given `entity`.
/// Exclusions are always assertions — every exclusion participates
/// in the post-detection filter.
///
/// [`Exclusion`]: AnnotationKind::Exclusion
pub fn is_excluded<M>(annotations: &[Annotation<M>], entity: &Entity<M>) -> bool
where
    M: Modality + Overlap,
{
    annotations.iter().any(|ann| {
        let AnnotationKind::Exclusion { target } = &ann.kind else {
            return false;
        };
        entity.location.overlaps(target)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modality::Text;
    use crate::primitive::BoundingBox;

    fn inclusion(start: usize, end: usize, strength: AnnotationStrength) -> Annotation<Text> {
        Annotation {
            name: None,
            kind: AnnotationKind::Inclusion {
                category: Some(EntityCategory::PersonalIdentity),
                entity_kind: Some(EntityKind::PersonName),
                target: Text::new(start, end),
                strength,
            },
        }
    }

    fn exclusion(start: usize, end: usize) -> Annotation<Text> {
        Annotation {
            name: None,
            kind: AnnotationKind::Exclusion {
                target: Text::new(start, end),
            },
        }
    }

    fn hint() -> AnnotationStrength {
        AnnotationStrength::Hint { confidence: None }
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
        let ann = inclusion(0, 10, AnnotationStrength::Assert);
        let entity = ann.to_inclusion_entity().unwrap();
        assert_eq!(entity.location, Text::new(0, 10));
        assert!((entity.confidence.get() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn hint_inclusion_does_not_produce_entity() {
        let ann = inclusion(0, 10, hint());
        assert!(ann.to_inclusion_entity().is_none());
    }

    #[test]
    fn unclassified_assert_inclusion_falls_back_to_unresolved() {
        let ann = Annotation {
            name: None,
            kind: AnnotationKind::Inclusion {
                category: None,
                entity_kind: None,
                target: Text::new(0, 10),
                strength: AnnotationStrength::Assert,
            },
        };
        let entity = ann.to_inclusion_entity().unwrap();
        assert_eq!(entity.category, EntityCategory::Unresolved);
        assert_eq!(entity.entity_kind, EntityKind::Unresolved);
    }

    #[test]
    fn to_inclusion_entity_returns_none_for_exclusion() {
        let ann = exclusion(0, 4);
        assert!(ann.to_inclusion_entity().is_none());
    }

    #[test]
    fn assert_inclusion_image() {
        use crate::modality::Image;
        let bbox = BoundingBox::new(0.0, 0.0, 10.0, 10.0);
        let ann: Annotation<Image> = Annotation {
            name: Some("face".into()),
            kind: AnnotationKind::Inclusion {
                category: Some(EntityCategory::PersonalIdentity),
                entity_kind: Some(EntityKind::PersonName),
                target: Image::new(bbox),
                strength: AnnotationStrength::Assert,
            },
        };
        let entity = ann.to_inclusion_entity().unwrap();
        assert_eq!(entity.location.bounding_box, bbox);
    }

    #[test]
    fn exclusion_by_location_overlap() {
        let anns = vec![exclusion(5, 15)];
        let entity = test_entity(10, 20);
        assert!(is_excluded(&anns, &entity));
    }

    #[test]
    fn exclusion_by_location_no_overlap() {
        let anns = vec![exclusion(0, 5)];
        let entity = test_entity(10, 20);
        assert!(!is_excluded(&anns, &entity));
    }
}
