//! [`NlpDetection`]: NER-specific knobs in the workflow detection
//! config.
//!
//! Cross-recognizer hints (`entity_kinds`, `confidence_threshold`)
//! live directly on [`Detection`] because every recognizer honors
//! them. This struct exists for any future NER-specific knobs and
//! to carry the enable/disable toggle so operators can opt the
//! recognizer in or out independently.
//!
//! [`Detection`]: crate::detection::Detection

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// NER-specific detection settings.
#[derive(Debug, Clone, PartialEq, Eq)]
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct NlpDetection {
    /// Enable this recognizer. When `false`, the recognizer is
    /// neither built nor dispatched, but the config is preserved
    /// so operators can toggle without losing it. Defaults to
    /// `true`.
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Backend selection for the NER pipeline. Defaults to
    /// [`NlpBackend::Noop`] so a baseline runtime works without
    /// any inference service configured; switch to
    /// [`NlpBackend::Bento`] to call the externalized
    /// `inference-gliner` service.
    ///
    /// [`NlpBackend::Noop`]: crate::detection::NlpBackend::Noop
    /// [`NlpBackend::Bento`]: crate::detection::NlpBackend::Bento
    #[serde(default)]
    pub backend: NlpBackend,
}

impl Default for NlpDetection {
    fn default() -> Self {
        Self {
            enabled: true,
            backend: NlpBackend::default(),
        }
    }
}

/// Backend the [`NlpRecognizer`] dispatches to.
///
/// The enum is always parseable regardless of compiled features —
/// selecting [`NlpBackend::Bento`] on a build without the `bento`
/// feature surfaces as a clear runtime error at recognizer
/// construction rather than a deserialisation failure, so config
/// files stay portable across deployments.
///
/// [`NlpRecognizer`]: crate::detection::nlp::NlpRecognizer
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum NlpBackend {
    /// No-op backend — produces zero entities. The default; used by
    /// tests and by deployments that detect via patterns/LLM only.
    #[default]
    Noop,

    /// Externalised [`BentoNerBackend`] — calls the
    /// `inference-gliner` Bento over HTTP. Requires the runtime to
    /// be built with the `bento` feature.
    ///
    /// [`BentoNerBackend`]: nvisy_nlp::ner::BentoNerBackend
    Bento {
        /// Base URL of the `inference-gliner` Bento (for example
        /// `http://localhost:3000` or `http://inference-gliner:3000`
        /// inside a docker-compose network).
        base_url: String,
    },
}

fn default_true() -> bool {
    true
}
