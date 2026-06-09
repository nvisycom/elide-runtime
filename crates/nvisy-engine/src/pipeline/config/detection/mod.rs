//! Detection config: deployment-time `[detection.*]` configuration
//! the engine builds a [`RecognizerRegistry`] from at startup, plus
//! the per-request [`Detection`] plan node.
//!
//! Pattern detection is always-on (default registry + custom extras
//! aren't yet plan-configurable). NER is opt-in via
//! `[detection.ner]`. LLM and VLM sections are not currently wired
//! — those modules are parked pending rework to implement
//! [`EntityRecognizer<M>`] directly.
//!
//! [`RecognizerRegistry`]: nvisy_toolkit::detection::RecognizerRegistry
//! [`EntityRecognizer<M>`]: nvisy_core::recognition::EntityRecognizer

mod ner;
mod pattern;
mod plan;

#[cfg(not(feature = "bento"))]
use nvisy_core::Error;
use nvisy_core::Result;
use nvisy_core::entity::EntityKind;
use nvisy_ner::NerRecognizer;
use nvisy_ner::backend::NoopBackend;
#[cfg(feature = "bento")]
use nvisy_ner::backend::{BentoBackend, BentoParams};
use nvisy_pattern::{PatternRecognizer, PatternRegistry};
use nvisy_toolkit::detection::RecognizerRegistry;

pub use self::ner::{NerBackend, NerDetection};
pub use self::pattern::PatternDetection;
pub use self::plan::Detection;

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
    /// Build the [`RecognizerRegistry`] from each opted-in section,
    /// inserting the concrete recognizer directly so no `dyn`
    /// erasure is required outside the registry's internal storage.
    ///
    /// Pattern detection is always-on: even when `cfg.pattern` is
    /// `None`, a pattern recognizer with the shipped default registry
    /// is registered. NER is opt-in via `cfg.ner`.
    ///
    /// # Errors
    ///
    /// Returns the first construction error encountered — pattern
    /// compile failure, NER backend init failure, or a
    /// config-selected backend whose feature wasn't compiled in.
    pub fn build(&self) -> Result<RecognizerRegistry> {
        let mut reg = RecognizerRegistry::new();

        let pattern_cfg = self.pattern.clone().unwrap_or_default();
        if pattern_cfg.enabled {
            let recognizer = PatternRecognizer::builder()
                .with_registry(PatternRegistry::builtin())
                .build()?;
            reg = reg.add_text_recognizer(recognizer);
        }

        if let Some(ner_cfg) = self.ner.as_ref().filter(|c| c.enabled) {
            reg = match &ner_cfg.backend {
                NerBackend::Noop => {
                    let recognizer = NerRecognizer::builder()
                        .with_name(NER_RECOGNIZER_NAME)
                        .with_engine(NoopBackend)
                        .with_supported_kinds(default_text_kinds())
                        .build()?;
                    reg.add_text_recognizer(recognizer)
                }

                #[cfg(feature = "bento")]
                NerBackend::Bento { base_url } => {
                    let backend = BentoBackend::new(BentoParams::new(base_url.clone()))?;
                    let recognizer = NerRecognizer::builder()
                        .with_name(NER_RECOGNIZER_NAME)
                        .with_engine(backend)
                        .with_supported_kinds(default_text_kinds())
                        .build()?;
                    reg.add_text_recognizer(recognizer)
                }

                #[cfg(not(feature = "bento"))]
                NerBackend::Bento { .. } => {
                    return Err(Error::validation(
                        "NerBackend::Bento requires the `bento` feature",
                        "ner",
                    ));
                }
            };
        }

        Ok(reg)
    }
}

/// Default kind allowlist for the engine-side NER recognizer.
///
/// Every defined [`EntityKind`] except those that only surface in
/// images (biometric templates, visual elements). The zero-shot model
/// is fed this list as "look for any of these"; centralised
/// post-filtering at the dispatch layer narrows further per call.
///
/// [`EntityKind`]: nvisy_core::entity::EntityKind
fn default_text_kinds() -> Vec<EntityKind> {
    EntityKind::all()
        .filter(|k| !k.is_biometric() && !k.is_visual())
        .collect()
}
