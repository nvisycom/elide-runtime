//! [`SuppressAction`]: payload of an [`Action::Suppress`] rule.
//!
//! Today carries an optional `reason` string that surfaces in the
//! audit entry. Designed as a struct (not a unit type) so future
//! fields (`expires`, `severity`, …) can land without a wire-break.
//!
//! [`Action::Suppress`]: super::rule::Action::Suppress

use hipstr::HipStr;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Payload for the `suppress` action.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
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
