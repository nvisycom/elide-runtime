//! Detection result types.
//!
//! A [`DetectionResult`] groups the output of a detection pass as a
//! first-class type, carrying the detected entities alongside pipeline
//! and policy metadata.

pub mod annotation;
pub mod classification;

pub use annotation::{Annotation, AnnotationKind, AnnotationLabel};
pub use classification::ClassificationResult;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use nvisy_core::path::ContentSource;

use crate::entity::Entity;

/// Sensitivity classification assigned to a document or content region.
///
/// Drives downstream policy: rules can be scoped to specific sensitivity
/// levels via [`RuleCondition`](crate::policy::RuleCondition).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "jsonschema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum SensitivityLevel {
    /// No sensitive data detected or all data is publicly available.
    Public,
    /// Internal use only — not intended for external distribution.
    Internal,
    /// Contains sensitive data requiring access controls.
    Confidential,
    /// Highly sensitive — regulated data requiring strict controls.
    Restricted,
}

/// Types that can be submitted for sensitive data detection.
pub trait Detectable: Send + Sync {
    /// Content as text for text-based detection.
    fn text_content(&self) -> Option<&str>;
    /// Binary content for image/audio/video detection.
    fn binary_content(&self) -> Option<&[u8]>;
    /// MIME type of the content.
    fn mime_type(&self) -> Option<&str>;
    /// Source identity for lineage.
    fn source(&self) -> &ContentSource;
}

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
    /// Identifier of the pipeline run that produced this result.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run_id: Option<Uuid>,
    /// Processing time in milliseconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    /// Overall sensitivity classification derived from the detected entities.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sensitivity_level: Option<SensitivityLevel>,
    /// Re-identification risk score in the range `[0.0, 1.0]`.
    ///
    /// Estimates the likelihood that a data subject could be re-identified
    /// from the entities remaining after redaction. Computed post-transform.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub risk_score: Option<f64>,
}
