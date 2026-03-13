//! Biometric reference data for identity verification.

mod face;
mod voice;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub use self::face::FaceData;
pub use self::voice::VoiceData;

/// Biometric identity verification variants.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BiometricVariant {
    /// Reference face for identity matching.
    Face(FaceData),
    /// Reference voiceprint for speaker identification.
    Voice(VoiceData),
}
