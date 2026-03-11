//! Detection output: per-source result of a detection pass.

use std::time::Duration;

use nvisy_core::content::ContentSource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_with::{DurationMicroSeconds, serde_as};
use uuid::Uuid;

use super::Entities;

/// The output of a detection pass over a single content source.
#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DetectionOutput {
    /// Content source identity and lineage.
    pub source: ContentSource,
    /// Entities detected in the content.
    pub entities: Entities,
    /// Identifier of the policy that governed detection.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub policy_id: Option<Uuid>,
    /// Processing time.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde_as(as = "Option<DurationMicroSeconds>")]
    #[schemars(with = "Option<u64>")]
    pub duration: Option<Duration>,
    /// Non-fatal errors or warnings encountered during detection.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub errors: Vec<String>,
}

impl DetectionOutput {
    /// The unique identifier for this detection output (delegates to `source.as_uuid()`).
    pub fn id(&self) -> Uuid {
        self.source.as_uuid()
    }

    /// Create a new detection output for the given source.
    pub fn new(source: ContentSource, entities: impl Into<Entities>) -> Self {
        Self {
            source,
            entities: entities.into(),
            policy_id: None,
            duration: None,
            errors: Vec::new(),
        }
    }

    /// Set the policy identifier.
    pub fn with_policy_id(mut self, policy_id: Uuid) -> Self {
        self.policy_id = Some(policy_id);
        self
    }

    /// Set the processing duration.
    pub fn with_duration(mut self, duration: Duration) -> Self {
        self.duration = Some(duration);
        self
    }
}
