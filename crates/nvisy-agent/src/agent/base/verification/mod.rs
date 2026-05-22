//! Shared LLM-verification output shape.
//!
//! Both [`CvVerifyAgent`] and [`NerVerifyAgent`] prompt an LLM with a list
//! of proposed entities and ask it to vote confirm/correct/reject
//! per entry. The verdict shape is identical across modalities;
//! only the per-modality location update (bounding box vs. text
//! offsets) differs, and that is each verifier's concern when
//! applying a verdict to an entity.
//!
//! Confirmed entities are omitted from [`VerificationOutput`]; only
//! changed (corrected or rejected) entries appear.
//!
//! [`CvVerifyAgent`]: crate::agent::cv::CvVerifyAgent
//! [`NerVerifyAgent`]: crate::agent::ner::NerVerifyAgent

use nvisy_ontology::entity::{EntityCategory, EntityKind};
use nvisy_ontology::primitive::{BoundingBox, Confidence};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Whether a proposed entity was corrected or rejected by the
/// verifier LLM.
#[derive(Debug, Clone, PartialEq, Eq)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum VerificationStatus {
    /// The entity value or classification was corrected.
    Corrected,
    /// The entity was rejected as a false positive.
    Rejected,
}

/// A single entity whose status changed during LLM verification.
///
/// The `id` field is the index into the proposed entity slice, so
/// the caller can diff against the original list.
///
/// Modality-specific fields (`bbox` for CV, future text fields for
/// NER) are optional — the verifier only fills in what its modality
/// supports.
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
    /// Verifier confidence in the verdict.
    pub confidence: Confidence,
    /// Corrected bounding box (CV verifier only; present when
    /// `status` is `Corrected` and the modality is image-based).
    pub bbox: Option<BoundingBox>,
    /// Optional rationale for the correction or rejection.
    pub reason: Option<String>,
}

/// Verification output containing only entities whose status
/// changed.
///
/// Entities not present in this list are implicitly confirmed.
#[derive(Debug, Clone, Default, PartialEq)]
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct VerificationOutput {
    /// Only entities that were corrected or rejected.
    pub entities: Vec<VerifiedEntity>,
}
