//! Structured output types for OCR entity verification.

use nvisy_core::math::BoundingBox;
use nvisy_ontology::entity::{EntityCategory, EntityKind};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Whether a proposed entity was corrected or rejected by the VLM.
///
/// Entities that are confirmed are omitted from the output entirely —
/// only changed entities appear in [`VerificationOutput`].
#[derive(Debug, Clone, PartialEq, Eq)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum VerificationStatus {
    /// The entity value or classification was corrected.
    Corrected,
    /// The entity was rejected as a false positive.
    Rejected,
}

/// A single entity whose status changed during VLM verification.
///
/// The `id` field is the index into the proposed entity slice, so the
/// caller can diff against the original list.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, JsonSchema)]
pub struct VerifiedEntity {
    /// Index into the proposed entity list.
    pub id: usize,
    /// Whether this entity was corrected or rejected.
    pub status: VerificationStatus,
    /// Corrected category (present when `status` is `Corrected`).
    pub category: Option<EntityCategory>,
    /// Corrected entity type (present when `status` is `Corrected`).
    pub entity_type: Option<EntityKind>,
    /// Corrected value (present when `status` is `Corrected`).
    pub value: Option<String>,
    /// VLM confidence in the verdict (0.0..=1.0).
    pub confidence: f64,
    /// Corrected bounding box (present when `status` is `Corrected`).
    pub bbox: Option<BoundingBox>,
    /// Optional rationale for the correction or rejection.
    pub reason: Option<String>,
}

/// Verification output containing only entities whose status changed.
///
/// Entities not present in this list are implicitly confirmed.
#[derive(Debug, Clone, Default, PartialEq)]
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct VerificationOutput {
    /// Only entities that were corrected or rejected.
    pub entities: Vec<VerifiedEntity>,
}
