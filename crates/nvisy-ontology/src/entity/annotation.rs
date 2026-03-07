//! Annotation types for pre-identified regions and classification labels.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumString};

use super::{EntityCategory, EntityKind, Location};

/// The kind of annotation applied to a content region.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Display)]
#[derive(EnumString, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
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
