//! [`TabularRedaction`]: the operator spec a tabular-modality policy
//! rule carries.
//!
//! No tabular operators exist yet in elide, but the enum exists so
//! policy schemas can be parameterised uniformly over all four
//! modalities. Only [`TabularRedaction::Custom`] is implementable
//! today.

use elide_core::redaction::OperatorId;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Operator spec a `redact` tabular rule carries.
#[derive(Debug, Clone, PartialEq, Eq)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TabularRedaction {
    /// Reserved for a future tabular registry; not invokable today.
    Custom {
        /// Id under which the operator would be registered.
        #[schemars(with = "crate::schema::OperatorIdSchema")]
        id: OperatorId,
    },
}
