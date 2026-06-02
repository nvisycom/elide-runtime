//! NER-recognizer wiring.
//!
//! The engine registers a [`nvisy_ner::recognition::GlinerRecognizer`]
//! directly — it already implements
//! [`nvisy_core::Recognizer<Text>`](nvisy_core::Recognizer). No
//! engine-side adapter wrapper.
//!
//! [`NerDetection`] is the operator-facing config; the
//! [`build_recognizer`] free function turns that config into the
//! built recognizer wrapped in an `Arc`. Backend selection is
//! config-driven via the local [`NerBackend`] enum which produces
//! a [`GlinerBackend`](nvisy_ner::backend::GlinerBackend) impl.
//! [`NoopBackend`] is the baseline; [`BentoBackend`] (feature
//! `bento`) is the externalised inference service.
//!
//! Post-filtering (entity-kind allowlist, score threshold) is
//! applied centrally at the detection layer, not inside this
//! recognizer.
//!
//! Per-call language hinting is the caller's job today — the
//! engine's dispatch loop forwards `ctx.language` if set; otherwise
//! the underlying GLiNER backend is multilingual and works without
//! a hint. A future shared `NlpEngine` pass will run language
//! detection once per scan and stamp the result on the artifact.
//!
//! [`NoopBackend`]: nvisy_ner::backend::NoopBackend
//! [`BentoBackend`]: nvisy_ner::backend::BentoBackend

use std::sync::Arc;

use nvisy_core::{Recognizer, Result};
use nvisy_ner::backend;
use nvisy_ner::recognition::{GlinerRecognizer, NerModelConfiguration};
use nvisy_ontology::entity::EntityKind;
use nvisy_ontology::modality::Text;

use crate::pipeline::{NerBackend, NerDetection};

/// Stable name the recognizer registers under (also surfaced in
/// trail provenance).
const RECOGNIZER_NAME: &str = "ner";

/// Build the engine-side NER recognizer from a [`NerDetection`]
/// config.
///
/// # Errors
///
/// Returns an error if the selected backend cannot be constructed,
/// or if the config selects a backend whose feature wasn't compiled
/// in.
pub fn build_recognizer(cfg: &NerDetection) -> Result<Arc<dyn Recognizer<Text>>> {
    let recognizer = match &cfg.backend {
        NerBackend::Noop => GlinerRecognizer::new(
            RECOGNIZER_NAME,
            backend::NoopBackend,
            default_text_kinds(),
            NerModelConfiguration::default(),
        ),

        #[cfg(feature = "bento")]
        NerBackend::Bento { base_url } => {
            let backend = backend::BentoBackend::new(backend::BentoParams::new(base_url.clone()))?;
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

/// Default kind allowlist for the engine-side NER recognizer.
///
/// Every defined [`EntityKind`] except those that only surface in
/// images (biometric templates, visual elements). The zero-shot
/// model is fed this list as "look for any of these"; centralized
/// post-filtering at the engine layer narrows further per call.
fn default_text_kinds() -> Vec<EntityKind> {
    EntityKind::all()
        .filter(|k| !k.is_biometric() && !k.is_visual())
        .collect()
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
        let recognizer = build_recognizer(&cfg).expect("builds");
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
        match build_recognizer(&cfg) {
            Ok(_) => panic!("Bento should not build without `bento` feature"),
            Err(e) => assert!(
                e.to_string().contains("`bento` feature"),
                "error should mention the bento feature: {e}",
            ),
        }
    }
}
