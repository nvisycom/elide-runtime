//! Detection result types.
//!
//! A [`DetectionResult`] groups the output of a detection pass as a
//! first-class type, carrying the detected entities alongside pipeline
//! and policy metadata.

mod annotation;
mod classification;
mod sensitivity;

pub use annotation::{Annotation, AnnotationKind, AnnotationLabel, AnnotationScope};
pub use classification::ClassificationResult;
pub use sensitivity::{Sensitivity, SensitivityLevel};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use nvisy_core::path::ContentSource;

use crate::entity::Entity;

/// The output of a detection pass over a single content source.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "jsonschema", derive(schemars::JsonSchema))]
pub struct DetectionResult {
    /// Content source identity and lineage.
    #[serde(flatten)]
    pub source: ContentSource,
    /// Entities detected in the content.
    pub entities: Vec<Entity>,
    /// Identifier of the policy that governed detection.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub policy_id: Option<Uuid>,
    /// Processing time in milliseconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    /// Overall sensitivity assessment derived from the detected entities.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sensitivity: Option<Sensitivity>,
}
