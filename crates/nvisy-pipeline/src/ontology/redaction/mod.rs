//! Redaction specifications, records, reviews, and summaries.
//!
//! - [`RedactionSpec`] — describes *how* to redact (method + config params).
//! - [`Redaction`] — records a redaction decision for a specific entity.
//! - [`ReviewDecision`] / [`ReviewStatus`] — human-in-the-loop review.
//! - [`RedactionSummary`] — per-source redaction counts.
//! - [`Redactable`] trait — types that produce redaction decisions.

mod review;
mod spec;
mod summary;
mod trait_;

pub use review::{ReviewDecision, ReviewStatus};
pub use spec::{
    AudioRedactionSpec, ImageRedactionSpec, RedactionSpec, TextRedactionSpec,
    DEFAULT_BLOCK_COLOR, DEFAULT_BLUR_SIGMA, DEFAULT_MASK_CHAR, DEFAULT_PIXELATE_BLOCK_SIZE,
};
pub use summary::RedactionSummary;
pub use trait_::Redactable;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use nvisy_codec::transform::RedactionOutput;
use nvisy_core::path::ContentSource;

/// A redaction decision recording how a specific entity was (or will be) redacted.
///
/// Each `Redaction` is linked to exactly one entity via `entity_id`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[derive(schemars::JsonSchema)]
pub struct Redaction {
    /// Content source identity and lineage.
    #[serde(flatten)]
    pub source: ContentSource,
    /// Identifier of the entity being redacted.
    pub entity_id: Uuid,
    /// Redaction output recording the method used and its result data.
    pub output: RedactionOutput,
    /// The original sensitive value, retained for audit purposes.
    pub original_value: String,
    /// Detection confidence that led to this redaction.
    pub confidence: f64,
    /// Identifier of the policy rule that triggered this redaction.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub policy_rule_id: Option<Uuid>,
    /// Whether the redaction has been applied to the output content.
    pub applied: bool,
    /// Version of this redaction record (starts at 1, incremented on modification).
    pub version: u32,
    /// Human review decision, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub review: Option<ReviewDecision>,
}

impl Redaction {
    /// Create a new pending redaction for the given entity.
    pub fn new(
        entity_id: Uuid,
        output: impl Into<RedactionOutput>,
        original_value: impl Into<String>,
        confidence: f64,
    ) -> Self {
        Self {
            source: ContentSource::new(),
            entity_id,
            output: output.into(),
            original_value: original_value.into(),
            confidence,
            policy_rule_id: None,
            applied: false,
            version: 1,
            review: None,
        }
    }

    /// Associate this redaction with the policy rule that triggered it.
    pub fn with_policy_rule_id(mut self, id: Uuid) -> Self {
        self.policy_rule_id = Some(id);
        self
    }
}
