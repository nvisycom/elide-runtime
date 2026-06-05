//! [`ImageRedaction`]: the operator spec an image-modality policy
//! rule carries.
//!
//! The toolkit ships no built-in image operators yet, so the only
//! variant today is [`ImageRedaction::Custom`] — deployments
//! register their own image anonymisers (blur, pixelate,
//! blackbox, …) on the [`RedactionRegistry<Image>`] and reference
//! them by id.
//!
//! [`RedactionRegistry<Image>`]: nvisy_toolkit::redaction::RedactionRegistry

use nvisy_core::modality::Image;
use nvisy_toolkit::redaction::AnonymizerId;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Operator spec a `redact` image rule carries.
#[derive(Debug, Clone, PartialEq, Eq)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ImageRedaction {
    /// Look up a deployment-registered custom operator by id.
    Custom {
        /// Id under which the operator was registered in the
        /// [`RedactionRegistry<Image>`].
        ///
        /// [`RedactionRegistry<Image>`]: nvisy_toolkit::redaction::RedactionRegistry
        id: AnonymizerId<Image>,
    },
}
