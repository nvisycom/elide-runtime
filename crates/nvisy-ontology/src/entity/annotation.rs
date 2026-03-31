//! Annotation types for pre-identified regions and classification labels.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumString};

use super::{Entities, Entity, EntityCategory, EntityKind, Location, RecognitionMethod};
use crate::entity::Overlap;

/// The kind of annotation applied to a content region.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Display)]
#[derive(EnumString, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
#[non_exhaustive]
pub enum AnnotationKind {
    /// Pre-identified sensitive region that should be treated as a detection.
    Inclusion,
    /// Known-safe region that detection should skip.
    Exclusion,
    /// Classification label attached to a document or region.
    Label,
}

/// The scope to which an annotation label applies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Display)]
#[derive(EnumString, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
#[non_exhaustive]
pub enum AnnotationScope {
    /// Label applies to the entire document.
    Document,
    /// Label applies to a specific page.
    Page,
    /// Label applies to a specific region or element.
    Region,
}

/// A classification label attached to a document or region.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AnnotationLabel {
    /// Label name (e.g. `"contains-phi"`, `"gdpr-request"`).
    pub name: String,
    /// Scope of the label.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<AnnotationScope>,
    /// Confidence of the label assignment.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f64>,
}

impl AnnotationLabel {
    /// Create a new label with the given name.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            scope: None,
            confidence: None,
        }
    }

    /// Set the scope.
    pub fn with_scope(mut self, scope: AnnotationScope) -> Self {
        self.scope = Some(scope);
        self
    }

    /// Set the confidence.
    pub fn with_confidence(mut self, confidence: f64) -> Self {
        self.confidence = Some(confidence);
        self
    }
}

/// A user-provided or upstream annotation on a content region.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Annotation {
    /// What kind of annotation this is.
    pub kind: AnnotationKind,
    /// Entity category, if applicable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<EntityCategory>,
    /// Entity kind, if applicable.
    #[serde(rename = "entity_type", skip_serializing_if = "Option::is_none")]
    pub entity_kind: Option<EntityKind>,
    /// The annotated text or value.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    /// Modality-specific location of the annotated region.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<Location>,
    /// Classification labels attached to this annotation.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub labels: Vec<AnnotationLabel>,
}

/// A collection of [`Annotation`]s.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
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

    /// Check whether the given entity falls within any exclusion annotation.
    ///
    /// An entity is excluded if:
    /// - An exclusion annotation has the same value (exact match), or
    /// - An exclusion annotation has a text location that overlaps the entity's.
    pub fn is_excluded(&self, entity: &Entity) -> bool {
        for ann in &self.0 {
            if ann.kind != AnnotationKind::Exclusion {
                continue;
            }
            if let Some(ref excl_val) = ann.value
                && *excl_val == entity.value
            {
                return true;
            }
            if let (Some(Location::Text(entity_loc)), Some(Location::Text(excl_loc))) =
                (&entity.location, &ann.location)
                && entity_loc.overlaps(excl_loc)
            {
                return true;
            }
        }
        false
    }

    /// Convert inclusion annotations into entities and add them to the collection.
    pub fn apply_inclusions(&self, entities: &mut Entities) {
        for ann in &self.0 {
            if ann.kind != AnnotationKind::Inclusion {
                continue;
            }
            let category = match ann.category {
                Some(c) => c,
                None => continue,
            };
            let entity_kind = match ann.entity_kind {
                Some(ek) => ek,
                None => continue,
            };
            let value = ann.value.clone().unwrap_or_default();
            let mut builder = Entity::builder()
                .with_category(category)
                .with_entity_kind(entity_kind)
                .with_value(value)
                .with_recognition_methods(vec![RecognitionMethod::manual_anonymous()])
                .with_confidence(1.0);
            if let Some(ref loc) = ann.location {
                builder = builder.with_location(loc.clone());
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
