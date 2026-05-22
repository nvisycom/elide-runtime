//! [`RedactorTtsConfig`]: TTS provider config used to synthesize
//! audio replacements for redacted spans.
//!
//! Lives under [`RedactorDefaults`] because TTS is a redaction-codec
//! concern, not a separate phase: when an audio document is
//! redacted, the redaction apply step synthesizes replacement audio
//! (silence, bleeps, or a "redacted" voice) using this provider.
//!
//! [`RedactorDefaults`]: super::RedactorDefaults

use nvisy_agent::audio::TtsProvider;
use nvisy_agent::audio::tts::TtsConfig;
use serde::{Deserialize, Serialize};

/// `[redactor.tts]` config bundle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedactorTtsConfig {
    /// Enable TTS-based audio redaction. When `false`, the audio
    /// codec falls back to a non-TTS replacement (silence, beep)
    /// instead of synthesized speech. Defaults to `true`.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// TTS provider selection + credentials.
    pub provider: TtsProvider,
    /// TTS sampling/retry parameters.
    #[serde(default)]
    pub agent: TtsConfig,
}

fn default_true() -> bool {
    true
}
