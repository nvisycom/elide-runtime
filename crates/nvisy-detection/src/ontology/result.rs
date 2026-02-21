//! Detection result types.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use nvisy_core::path::ContentSource;

use super::entity::Entity;

/// The output of a detection pass over a single content source.
#[derive(Debug, Clone, Serialize, Deserialize)]
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
}
