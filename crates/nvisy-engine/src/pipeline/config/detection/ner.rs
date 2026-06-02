//! [`NerDetection`] + [`NerBackend`]: NER-recognizer settings.

use std::sync::Arc;

use nvisy_core::{Recognizer, Result};
use nvisy_ner::backend;
use nvisy_ner::recognition::{GlinerRecognizer, NerModelConfiguration};
use nvisy_ontology::entity::EntityKind;
use nvisy_ontology::modality::Text;
use serde::{Deserialize, Serialize};

/// Stable name the NER recognizer registers under (surfaced in trail
/// provenance on emitted entities).
const RECOGNIZER_NAME: &str = "ner";

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

    /// Externalised [`BentoBackend`]
    /// — calls the `inference-gliner` Bento over HTTP. Requires
    /// `nvisy-engine` to be built with the `bento` feature.
    ///
    /// [`BentoBackend`]: nvisy_ner::backend::BentoBackend
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

impl NerDetection {
    /// Build the engine-side NER recognizer.
    ///
    /// # Errors
    ///
    /// Returns an error if the selected backend cannot be
    /// constructed, or if the config selects a backend whose feature
    /// wasn't compiled in.
    pub fn build(&self) -> Result<Arc<dyn Recognizer<Text>>> {
        let recognizer = match &self.backend {
            NerBackend::Noop => GlinerRecognizer::new(
                RECOGNIZER_NAME,
                backend::NoopBackend,
                default_text_kinds(),
                NerModelConfiguration::default(),
            ),

            #[cfg(feature = "bento")]
            NerBackend::Bento { base_url } => {
                let backend =
                    backend::BentoBackend::new(backend::BentoParams::new(base_url.clone()))?;
                GlinerRecognizer::new(
                    RECOGNIZER_NAME,
                    backend,
                    default_text_kinds(),
                    NerModelConfiguration::default(),
                )
            }

            #[cfg(not(feature = "bento"))]
            NerBackend::Bento { .. } => {
                return Err(nvisy_core::Error::validation(
                    "NerBackend::Bento requires nvisy-engine to be built with the `bento` feature",
                    "ner",
                ));
            }
        };
        Ok(Arc::new(recognizer))
    }
}

impl Default for NerDetection {
    fn default() -> Self {
        Self {
            enabled: true,
            backend: NerBackend::default(),
        }
    }
}

/// Default kind allowlist for the engine-side NER recognizer.
///
/// Every defined [`EntityKind`] except those that only surface in
/// images (biometric templates, visual elements). The zero-shot
/// model is fed this list as "look for any of these"; centralised
/// post-filtering at the dispatch layer narrows further per call.
fn default_text_kinds() -> Vec<EntityKind> {
    EntityKind::all()
        .filter(|k| !k.is_biometric() && !k.is_visual())
        .collect()
}

fn default_true() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use nvisy_core::{Context, TextData};

    use super::*;

    #[tokio::test]
    async fn build_noop_yields_no_entities() {
        let cfg = NerDetection {
            enabled: true,
            backend: NerBackend::Noop,
        };
        let recognizer = cfg.build().expect("builds");
        let ctx = Context::new(TextData::new("The quick brown fox"));
        let out = recognizer.recognize(&ctx).await.unwrap();
        assert!(out.is_empty());
    }

    #[cfg(not(feature = "bento"))]
    #[tokio::test]
    async fn bento_without_feature_errors_clearly() {
        let cfg = NerDetection {
            enabled: true,
            backend: NerBackend::Bento {
                base_url: "http://localhost:3000".to_owned(),
            },
        };
        match cfg.build() {
            Ok(_) => panic!("Bento should not build without `bento` feature"),
            Err(e) => assert!(
                e.to_string().contains("`bento` feature"),
                "error should mention the bento feature: {e}",
            ),
        }
    }
}
