//! [`SuppressAction`]: payload of an [`PolicyAction::Suppress`] rule.
//!
//! [`PolicyAction::Suppress`]: crate::PolicyAction::Suppress

use hipstr::HipStr;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Payload for the `suppress` action.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SuppressAction {
    /// Human-readable reason the entity is being suppressed. Surfaced
    /// verbatim in the audit entry so reviewers can tell apart
    /// "synthetic test data," "incident response," and "known false
    /// positive" without re-reading the policy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<String>")]
    pub reason: Option<HipStr<'static>>,
}
