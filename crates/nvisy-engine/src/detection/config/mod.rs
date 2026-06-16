//! Detection config: deployment-time `[detection.*]` configuration
//! describing which recognizers are available. The per-request
//! [`RecognizerRegistry`] is built from this template plus the
//! request's policy-supplied [`EntityLabelCatalog`].
//!
//! Pattern detection is always-on; the registry built per request
//! drops patterns whose label isn't in the request catalog. NER is
//! opt-in via `[detection.ner]`. LLM and VLM sections are not
//! currently wired — those modules are parked pending rework to
//! implement [`EntityRecognizer<M>`] directly.
//!
//! Each recognizer owns its own post-recognition processing
//! (boosting, deduplication-within-recognizer, validation post-pass).
//! The engine orchestrates recognizers; it does not orchestrate
//! recognizer-internal phases.
//!
//! [`RecognizerRegistry`]: nvisy_toolkit::detection::RecognizerRegistry
//! [`EntityRecognizer<M>`]: nvisy_core::recognition::EntityRecognizer

mod ner;
mod pattern;

#[cfg(not(feature = "bento"))]
use nvisy_core::Error;
use nvisy_core::Result;
use nvisy_core::entity::EntityLabelCatalog;
use nvisy_core::modality::Text;
use nvisy_ner::NerRecognizer;
use nvisy_ner::backend::NoopBackend;
#[cfg(feature = "bento")]
use nvisy_ner::backend::{BentoBackend, BentoParams};
use nvisy_pattern::PatternRecognizer;
use nvisy_toolkit::detection::RecognizerRegistry;

pub use self::ner::{NerBackend, NerDetection};
pub use self::pattern::PatternDetection;

/// Stable name the NER recognizer registers under (surfaced in trail
/// provenance on emitted entities).
const NER_RECOGNIZER_NAME: &str = "ner";

/// Configuration for the [`RecognizerRegistry`].
///
/// Each field maps to a `[detection.*]` section in `Nvisy.toml`.
/// Every field is `Option<_>` so missing sections are valid — `None`
/// means the recognizer is not loaded (or uses its always-on default
/// for `pattern`).
///
/// [`RecognizerRegistry`]: nvisy_toolkit::detection::RecognizerRegistry
#[derive(Debug, Clone, Default, PartialEq)]
#[derive(serde::Serialize, serde::Deserialize)]
pub struct DetectionConfig {
    /// `[detection.pattern]` — pattern recognizer config. `None` uses
    /// the shipped registry with default settings (the recognizer is
    /// always-on).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pattern: Option<PatternDetection>,
    /// `[detection.ner]` — NER recognizer config bundle.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ner: Option<NerDetection>,
}

impl DetectionConfig {
    /// Build a per-request [`RecognizerRegistry`] from each opted-in
    /// section, wiring the request `catalog` into recognizers that
    /// need it (NER's zero-shot label list, pattern filtering).
    ///
    /// Pattern detection is always-on; pattern emissions whose label
    /// isn't registered in `catalog` are filtered out at registry
    /// construction so unmatched patterns never run. NER is opt-in
    /// via `cfg.ner`; when enabled, the request catalog's labels are
    /// passed as the zero-shot label list.
    ///
    /// # Errors
    ///
    /// Returns the first construction error encountered — pattern
    /// compile failure, NER backend init failure, or a
    /// config-selected backend whose feature wasn't compiled in.
    pub fn build_for_request(&self, catalog: &EntityLabelCatalog) -> Result<RecognizerRegistry> {
        let mut reg = RecognizerRegistry::new();

        let pattern_cfg = self.pattern.clone().unwrap_or_default();
        if pattern_cfg.enabled {
            let builder = PatternRecognizer::builder()
                .with_builtin_patterns()
                .with_builtin_dictionaries()
                .filter_by_catalog(catalog);
            if !builder.is_empty() {
                reg = reg.with_recognizer::<Text>(builder.build_context_enhanced()?);
            }
        }

        if let Some(ner_cfg) = self.ner.as_ref().filter(|c| c.enabled) {
            let supported_labels = catalog.iter().map(|l| l.label_ref()).collect::<Vec<_>>();
            let recognizer = match &ner_cfg.backend {
                NerBackend::Noop => NerRecognizer::builder()
                    .with_name(NER_RECOGNIZER_NAME)
                    .with_engine(NoopBackend)
                    .with_supported_labels(supported_labels)
                    .build()?,

                #[cfg(feature = "bento")]
                NerBackend::Bento { base_url } => {
                    let backend = BentoBackend::new(BentoParams::new(base_url.clone()))?;
                    NerRecognizer::builder()
                        .with_name(NER_RECOGNIZER_NAME)
                        .with_engine(backend)
                        .with_supported_labels(supported_labels)
                        .build()?
                }

                #[cfg(not(feature = "bento"))]
                NerBackend::Bento { .. } => {
                    return Err(Error::validation(
                        "NerBackend::Bento requires the `bento` feature",
                        "ner",
                    ));
                }
            };
            reg = reg.with_recognizer::<Text>(recognizer);
        }

        Ok(reg)
    }
}
