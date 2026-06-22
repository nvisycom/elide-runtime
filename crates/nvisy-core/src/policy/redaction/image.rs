//! [`ImageRedaction`]: the operator spec an image-modality policy
//! rule carries.
//!
//! Elide ships no built-in image operators today, so the only
//! variant is [`ImageRedaction::Custom`] — deployments register
//! their own image operators (blur, pixelate, blackbox, …) and
//! reference them by [`OperatorId`].

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::schema::OperatorIdSchema;

/// Operator spec a `redact` image rule carries.
#[derive(Debug, Clone, PartialEq, Eq)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ImageRedaction {
    /// Look up a deployment-registered custom operator by id.
    Custom {
        /// Id under which the operator was registered.
        id: OperatorIdSchema,
    },
}
