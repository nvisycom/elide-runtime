//! Annotation types for pre-identified regions and classification labels.
//!
//! Annotations allow users and upstream systems to mark regions of content
//! before detection runs. They replace the previous `ManualAnnotation` type
//! with a unified model supporting three kinds: inclusions (pre-identified
//! sensitive regions), exclusions (known-safe regions to skip), and
//! classification labels.

use serde::{Deserialize, Serialize};

use crate::entity::{
    AudioLocation, EntityCategory, ImageLocation, TabularLocation, TextLocation, VideoLocation,
};

/// The kind of annotation applied to a content region.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "jsonschema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum AnnotationKind {
    /// Pre-identified sensitive region that should be treated as a detection.
    Inclusion,
    /// Known-safe region that detection should skip.
    Exclusion,
    /// Classification label attached to a document or region.
    Label,
}

/// A classification label attached to a document or region.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "jsonschema", derive(schemars::JsonSchema))]
pub struct AnnotationLabel {
    /// Label name (e.g. `"contains-phi"`, `"gdpr-request"`).
    pub name: String,
    /// Scope of the label: `"document"` or a region identifier.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
    /// Confidence of the label assignment.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f64>,
}

/// A user-provided or upstream annotation on a content region.
///
/// Replaces the previous `ManualAnnotation` with a unified type that
/// supports inclusions, exclusions, and classification labels.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "jsonschema", derive(schemars::JsonSchema))]
pub struct Annotation {
    /// What kind of annotation this is.
    pub kind: AnnotationKind,
    /// Entity category, if applicable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<EntityCategory>,
    /// Entity type label, if applicable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entity_type: Option<String>,
    /// The annotated text or value.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    /// Text location of the annotated region.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_location: Option<TextLocation>,
    /// Image location of the annotated region.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_location: Option<ImageLocation>,
    /// Tabular location of the annotated region.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tabular_location: Option<TabularLocation>,
    /// Audio location of the annotated region.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio_location: Option<AudioLocation>,
    /// Video location of the annotated region.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub video_location: Option<VideoLocation>,
    /// Classification labels attached to this annotation.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub labels: Vec<AnnotationLabel>,
}
