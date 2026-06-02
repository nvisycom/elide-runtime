//! [`SttExtractorConfig`]: `[extractor.stt]` config bundle.

use nvisy_agent::audio::SttProvider;
use nvisy_agent::audio::stt::SttConfig;
use serde::{Deserialize, Serialize};

/// `[extractor.stt]` config bundle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SttExtractorConfig {
    /// Enable this extractor. When `false`, the extractor is
    /// neither built nor dispatched, but the config is preserved
    /// so operators can toggle without losing it. Defaults to
    /// `true`.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// STT provider selection + connection settings.
    pub provider: SttProvider,
    /// STT sampling/retry parameters.
    #[serde(default)]
    pub agent: SttConfig,
}

fn default_true() -> bool {
    true
}
