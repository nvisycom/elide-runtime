//! [`SttExtractorConfig`] + the closed [`SttBackend`] selector enum.
//!
//! `[extraction.stt]` is the TOML section the engine reads at startup;
//! it picks one of the backends [`nvisy_stt`] ships. The build path
//! lives in the parent [`ExtractionConfig::build`].
//!
//! [`ExtractionConfig::build`]: super::ExtractionConfig::build

use serde::{Deserialize, Serialize};

/// `[extraction.stt]` config bundle.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SttExtractorConfig {
    /// Enable this extractor. When `false`, the extractor is neither
    /// built nor dispatched, but the config is preserved so operators
    /// can toggle without losing it. Defaults to `true`.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// STT backend selection + connection settings.
    #[serde(default)]
    pub backend: SttBackend,
}

/// Config-side selection of which STT [`Backend`] to construct.
///
/// The enum is always parseable regardless of compiled features;
/// future hosted backends (Whisper, Deepgram, AssemblyAI) plug in here
/// as additional variants.
///
/// [`Backend`]: nvisy_stt::SttBackend
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum SttBackend {
    /// No-op backend — produces zero transcription segments. The
    /// default; used in tests and in deployments that don't need STT.
    #[default]
    Noop,
}

fn default_true() -> bool {
    true
}
