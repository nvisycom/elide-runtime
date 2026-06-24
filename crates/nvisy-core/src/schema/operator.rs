//! [`OperatorIdSchema`]: wire shape for
//! [`elide_core::redaction::OperatorId`].

use elide_core::redaction::OperatorId;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Wire-shape proxy for [`elide_core::redaction::OperatorId`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(rename = "OperatorId")]
pub struct OperatorIdSchema {
    /// Stable operator name (e.g. `"mask"`, `"aes-gcm-encrypt"`).
    pub name: String,
    /// Operator version at the time it was applied.
    pub version: String,
}

impl From<OperatorIdSchema> for OperatorId {
    fn from(s: OperatorIdSchema) -> Self {
        OperatorId::new(s.name, s.version)
    }
}

impl From<OperatorId> for OperatorIdSchema {
    fn from(o: OperatorId) -> Self {
        Self {
            name: o.name.as_str().to_owned(),
            version: o.version.as_str().to_owned(),
        }
    }
}
