//! [`NerDetection`] + [`NerBackend`]: NER-recognizer settings.

use serde::{Deserialize, Serialize};

/// Backend selection for the NER recognizer.
///
/// Parseable regardless of compiled features — selecting
/// [`NerBackend::Bento`] on a build without the `bento` feature
/// surfaces as a clear runtime error at construction rather than a
/// deserialisation failure, so config files stay portable across
/// deployments.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum NerBackend {
    /// Returns no entities. The default — used by tests and by
    /// deployments that detect via patterns / LLM only.
    #[default]
    Noop,

    /// Externalised [`BentoBackend`](nvisy_ner::backend::BentoBackend)
    /// — calls the `inference-gliner` Bento over HTTP. Requires
    /// `nvisy-engine` to be built with the `bento` feature.
    Bento {
        /// Base URL of the `inference-gliner` Bento (for example
        /// `http://localhost:3000` or
        /// `http://inference-gliner:3000` inside a docker-compose
        /// network).
        base_url: String,
    },
}

/// NER-specific detection settings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NerDetection {
    /// Enable this recognizer. When `false`, the recognizer is
    /// neither built nor dispatched, but the config is preserved
    /// so operators can toggle without losing it. Defaults to
    /// `true`.
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Backend selection for the NER pipeline. Defaults to
    /// [`NerBackend::Noop`] so a baseline runtime works without
    /// any inference service configured.
    #[serde(default)]
    pub backend: NerBackend,
}

impl Default for NerDetection {
    fn default() -> Self {
        Self {
            enabled: true,
            backend: NerBackend::default(),
        }
    }
}

fn default_true() -> bool {
    true
}
