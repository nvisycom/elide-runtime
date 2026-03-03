//! Biometric reference data for identity verification.

mod face;
mod voice;

pub use face::FaceData;
pub use voice::VoiceData;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Biometric identity verification variants.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BiometricVariant {
    /// Reference face for identity matching.
    Face(FaceData),
    /// Reference voiceprint for speaker identification.
    Voice(VoiceData),
}
