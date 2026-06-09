//! [`OcrExtractorConfig`] + the [`OcrBackend`] selector enum.
//!
//! `[extraction.ocr]` is the TOML section the engine reads at startup;
//! it picks one of the backends [`nvisy_ocr`] ships. The build path
//! lives in the parent [`ExtractionConfig::build`].
//!
//! [`ExtractionConfig::build`]: super::ExtractionConfig::build

use serde::{Deserialize, Serialize};

/// `[extraction.ocr]` config bundle.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OcrExtractorConfig {
    /// Enable this extractor. When `false`, the extractor is neither
    /// built nor dispatched, but the config is preserved so operators
    /// can toggle without losing it. Defaults to `true`.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// OCR backend selection + connection settings.
    #[serde(default)]
    pub backend: OcrBackend,
}

/// Config-side selection of which OCR backend to construct.
///
/// The enum is always parseable regardless of compiled features —
/// selecting [`OcrBackend::Bento`] on a build without the `bento`
/// feature surfaces as a clear runtime error at construction instead
/// of a deserialisation failure, so config files stay portable across
/// deployments.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum OcrBackend {
    /// No-op backend — produces zero OCR results. The default; used
    /// in tests and in deployments that don't need OCR.
    #[default]
    Noop,

    /// Externalised Bento backend — calls the `inference-ocr`
    /// Bento over HTTP. Requires the runtime to be built with the
    /// `bento` feature.
    Bento {
        /// Base URL of the `inference-ocr` Bento.
        base_url: String,
    },
}

fn default_true() -> bool {
    true
}
