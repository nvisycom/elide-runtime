//! [`AuditAction`]: payload of an [`Action::Audit`] rule.
//!
//! Flags entities that need extra human attention without
//! transforming them. The detection pass already produces an
//! audit entry per matched entity; this action tags those entries
//! with a severity hint so downstream review tooling can prioritise
//! them. Designed as a struct (not a unit type) so future fields
//! (`category`, `notify`, `reviewer`, …) can land without a
//! wire-break.
//!
//! [`Action::Audit`]: super::rule::Action::Audit

use hipstr::HipStr;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Payload for the `audit` action.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AuditAction {
    /// Severity hint propagated into the audit entry — e.g.
    /// `"low"`, `"medium"`, `"high"`. Free-form for now; downstream
    /// review tooling decides how to render or filter.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<String>")]
    pub severity: Option<HipStr<'static>>,
}
