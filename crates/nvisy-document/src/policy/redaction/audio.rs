//! [`AudioRedaction`]: the operator spec an audio-modality policy
//! rule carries.
//!
//! The toolkit ships no built-in audio operators yet, so the only
//! variant today is [`AudioRedaction::Custom`] — deployments
//! register their own audio anonymisers (silence, white-noise,
//! beep, …) on the [`RedactionRegistry<Audio>`] and reference them
//! by id.
//!
//! [`RedactionRegistry<Audio>`]: nvisy_toolkit::redaction::RedactionRegistry

use nvisy_core::modality::Audio;
use nvisy_toolkit::redaction::AnonymizerId;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Operator spec a `redact` audio rule carries.
#[derive(Debug, Clone, PartialEq, Eq)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AudioRedaction {
    /// Look up a deployment-registered custom operator by id.
    Custom {
        /// Id under which the operator was registered in the
        /// [`RedactionRegistry<Audio>`].
        ///
        /// [`RedactionRegistry<Audio>`]: nvisy_toolkit::redaction::RedactionRegistry
        id: AnonymizerId<Audio>,
    },
}
