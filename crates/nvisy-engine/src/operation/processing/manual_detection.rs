//! Manual annotation detection.
//!
//! Converts user-provided inclusion [`Annotation`]s into full [`Entity`] objects
//! and collects exclusion annotations for downstream filtering.

use serde::Deserialize;

use nvisy_core::Error;
use nvisy_ontology::entity::{
    Annotation, AnnotationKind, DetectionMethod, Entity, Location,
};

use crate::operation::{Operation, ParallelContext};

/// Typed parameters for [`ManualDetection`].
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManualDetectionParams {}

/// An exclusion zone that detection should skip.
#[derive(Debug, Clone)]
pub struct Exclusion {
    /// Modality-specific location of the excluded region.
    pub location: Option<Location>,
    /// The annotated value, if any.
    pub value: Option<String>,
}

/// Output of [`ManualDetection::execute`].
#[derive(Debug)]
pub struct ManualOutput {
    /// Entities derived from inclusion annotations.
    pub entities: Vec<Entity>,
    /// Exclusion zones derived from exclusion annotations.
    pub exclusions: Vec<Exclusion>,
}

/// Converts each inclusion [`Annotation`] into a full [`Entity`] with
/// `DetectionMethod::Manual` and confidence 1.0.  Collects exclusion
/// annotations for downstream filtering.
pub struct ManualDetection;

impl ManualDetection {
    pub async fn connect(_params: ManualDetectionParams) -> Result<Self, Error> {
        Ok(Self)
    }

    pub async fn execute(
        &self,
        annotations: Vec<Annotation>,
    ) -> Result<ManualOutput, Error> {
        let mut entities = Vec::new();
        let mut exclusions = Vec::new();

        for ann in &annotations {
            match ann.kind {
                AnnotationKind::Inclusion => {
                    let category = match &ann.category {
                        Some(c) => c.clone(),
                        None => continue,
                    };
                    let entity_kind = match ann.entity_kind {
                        Some(ek) => ek,
                        None => continue,
                    };
                    let value = ann.value.clone().unwrap_or_default();

                    let mut entity = Entity::new(
                        category,
                        entity_kind,
                        value,
                        DetectionMethod::Manual,
                        1.0,
                    );
                    entity.location = ann.location.clone();
                    entities.push(entity);
                }
                AnnotationKind::Exclusion => {
                    exclusions.push(Exclusion {
                        location: ann.location.clone(),
                        value: ann.value.clone(),
                    });
                }
                _ => {}
            }
        }

        Ok(ManualOutput { entities, exclusions })
    }
}

impl Operation for ManualDetection {
    type Input = ParallelContext<Vec<Annotation>>;
    type Output = ParallelContext<ManualOutput>;

    async fn call(
        &self,
        input: Self::Input,
    ) -> Result<Self::Output, Error> {
        let result = self.execute(input.into_inner()).await?;
        Ok(ParallelContext::new(result))
    }
}

/// Check whether an entity falls within any exclusion zone.
///
/// An entity is excluded if:
/// - An exclusion has the same value (exact match), or
/// - An exclusion has a text location that overlaps the entity's text location.
pub fn is_excluded(entity: &Entity, exclusions: &[Exclusion]) -> bool {
    for excl in exclusions {
        // Value-based exclusion.
        if let Some(ref excl_val) = excl.value
            && *excl_val == entity.value
        {
            return true;
        }

        // Location-based exclusion (text overlap).
        if let (Some(Location::Text(entity_loc)), Some(Location::Text(excl_loc))) =
            (&entity.location, &excl.location)
            && entity_loc.overlaps(excl_loc)
        {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use nvisy_ontology::entity::{EntityCategory, EntityKind, TextLocation};

    fn make_entity(value: &str, start: usize, end: usize) -> Entity {
        Entity::new(
            EntityCategory::Pii,
            EntityKind::PersonName,
            value,
            DetectionMethod::Manual,
            1.0,
        )
        .with_location(TextLocation {
            start_offset: start,
            end_offset: end,
            ..Default::default()
        }.into())
    }

    fn make_exclusion_by_value(value: &str) -> Exclusion {
        Exclusion {
            location: None,
            value: Some(value.into()),
        }
    }

    fn make_exclusion_by_location(start: usize, end: usize) -> Exclusion {
        Exclusion {
            location: Some(TextLocation {
                start_offset: start,
                end_offset: end,
                ..Default::default()
            }.into()),
            value: None,
        }
    }

    #[test]
    fn is_excluded_by_value() {
        let entity = make_entity("John Doe", 0, 8);
        let exclusions = vec![make_exclusion_by_value("John Doe")];
        assert!(is_excluded(&entity, &exclusions));
    }

    #[test]
    fn is_excluded_by_overlapping_location() {
        let entity = make_entity("secret", 10, 16);
        let exclusions = vec![make_exclusion_by_location(8, 20)];
        assert!(is_excluded(&entity, &exclusions));
    }

    #[test]
    fn not_excluded_non_overlapping() {
        let entity = make_entity("secret", 10, 16);
        let exclusions = vec![make_exclusion_by_location(20, 30)];
        assert!(!is_excluded(&entity, &exclusions));
    }

    #[test]
    fn not_excluded_different_value() {
        let entity = make_entity("Alice", 0, 5);
        let exclusions = vec![make_exclusion_by_value("Bob")];
        assert!(!is_excluded(&entity, &exclusions));
    }

    #[tokio::test]
    async fn execute_collects_exclusions() {
        let action = ManualDetection;
        let annotations = vec![
            Annotation {
                kind: AnnotationKind::Inclusion,
                category: Some(EntityCategory::Pii),
                entity_kind: Some(EntityKind::PersonName),
                value: Some("Alice".into()),
                location: None,
                labels: vec![],
            },
            Annotation {
                kind: AnnotationKind::Exclusion,
                category: None,
                entity_kind: None,
                value: Some("safe-text".into()),
                location: None,
                labels: vec![],
            },
        ];
        let output = action.execute(annotations).await.unwrap();
        assert_eq!(output.entities.len(), 1);
        assert_eq!(output.exclusions.len(), 1);
        assert_eq!(output.exclusions[0].value.as_deref(), Some("safe-text"));
    }
}
