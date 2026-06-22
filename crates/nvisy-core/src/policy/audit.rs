//! [`AuditAction`]: payload of an [`Action::Audit`] rule.
//!
//! [`Action::Audit`]: super::rule::Action::Audit

use hipstr::HipStr;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Payload for the `audit` action.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AuditAction {
    /// Severity hint propagated into the audit entry — e.g.
    /// `"low"`, `"medium"`, `"high"`. Free-form for now; downstream
    /// review tooling decides how to render or filter.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<String>")]
    pub severity: Option<HipStr<'static>>,
}
