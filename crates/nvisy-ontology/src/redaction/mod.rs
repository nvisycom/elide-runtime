//! Redaction methods, specifications, outputs, and records.
//!
//! This module contains three layers of redaction types:
//!
//! 1. **Method** ([`RedactionMethod`]) — a plain tag enum naming a redaction
//!    strategy. Used as a lightweight identifier (e.g. in logs, serialized
//!    references, or when the caller only needs to know *which* algorithm).
//!
//! 2. **Spec** ([`RedactionSpec`]) — a data-carrying enum that describes a
//!    redaction request submitted to the engine: which method to apply and
//!    the configuration parameters it needs (mask char, blur sigma, key id,
//!    etc.). Used on [`PolicyRule`](crate::policy::PolicyRule) and
//!    [`Policy`](crate::policy::Policy).
//!
//! 3. **Output** ([`RedactionOutput`]) — a data-carrying enum that records
//!    what was actually done and the result data (replacement string,
//!    ciphertext, shifted date, etc.). Stored on [`Redaction`].
//!
//! All three are organized by modality:
//! - Text / tabular: [`TextRedactionMethod`], [`TextRedactionSpec`], [`TextRedactionOutput`]
//! - Image / video:  [`ImageRedactionMethod`], [`ImageRedactionSpec`], [`ImageRedactionOutput`]
//! - Audio:          [`AudioRedactionMethod`], [`AudioRedactionSpec`], [`AudioRedactionOutput`]

pub mod method;
pub mod output;
pub mod review;
pub mod spec;
pub mod summary;

pub use method::{
    AudioRedactionMethod, ImageRedactionMethod, RedactionMethod, TextRedactionMethod,
};
pub use output::{
    AudioRedactionOutput, ImageRedactionOutput, RedactionOutput, TextRedactionOutput,
};
pub use review::{ReviewDecision, ReviewStatus};
pub use spec::{AudioRedactionSpec, ImageRedactionSpec, RedactionSpec, TextRedactionSpec};
pub use summary::RedactionSummary;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use nvisy_core::path::ContentSource;

use crate::entity::Entity;
use crate::policy::Policy;

/// Types that produce redaction decisions.
pub trait Redactable {
    /// The entities detected in this content.
    fn entities(&self) -> &[Entity];
    /// The policy governing redaction.
    fn policy(&self) -> Option<&Policy>;
}

/// A redaction decision recording how a specific entity was (or will be) redacted.
///
/// Each `Redaction` is linked to exactly one [`Entity`](crate::entity::Entity)
/// via `entity_id`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "jsonschema", derive(schemars::JsonSchema))]
pub struct Redaction {
    /// Content source identity and lineage.
    #[serde(flatten)]
    pub source: ContentSource,
    /// Identifier of the entity being redacted.
    pub entity_id: Uuid,
    /// Redaction output recording the method used and its result data.
    pub output: RedactionOutput,
    /// The original sensitive value, retained for audit purposes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_value: Option<String>,
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
    pub fn new(entity_id: Uuid, output: impl Into<RedactionOutput>) -> Self {
        Self {
            source: ContentSource::new(),
            entity_id,
            output: output.into(),
            original_value: None,
            policy_rule_id: None,
            applied: false,
            version: 1,
            review: None,
        }
    }

    /// Record the original sensitive value for audit trail purposes.
    pub fn with_original_value(mut self, value: impl Into<String>) -> Self {
        self.original_value = Some(value.into());
        self
    }

    /// Associate this redaction with the policy rule that triggered it.
    pub fn with_policy_rule_id(mut self, id: Uuid) -> Self {
        self.policy_rule_id = Some(id);
        self
    }
}
