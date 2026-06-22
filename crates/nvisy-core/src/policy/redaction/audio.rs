//! [`AudioRedaction`]: the operator spec an audio-modality policy
//! rule carries.
//!
//! Elide ships [`elide::redaction::operators::Silence`] when the
//! `audio` feature is enabled, but it is not part of the policy wire
//! vocabulary — the spec stays at [`AudioRedaction::Custom`] so the
//! wire format does not bake a build-time feature into the schema.
//! Deployments wire `Silence` (or anything else) via [`OperatorId`].

use elide_core::redaction::OperatorId;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Operator spec a `redact` audio rule carries.
#[derive(Debug, Clone, PartialEq, Eq)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AudioRedaction {
    /// Look up a deployment-registered custom operator by id.
    Custom {
        /// Id under which the operator was registered.
        #[schemars(with = "crate::schema::OperatorIdSchema")]
        id: OperatorId,
    },
}
