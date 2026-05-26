//! [`Recognizers`]: the runtime-config-driven recognizer registry.
//!
//! Built once at engine startup from the `[recognizer.*]` config
//! sections, holds each opted-in recognizer behind an `Arc`, and
//! is shared across every pipeline run. Per-run workflow
//! [`Detection`] nodes pick from this registry by [`RecognizerKind`]
//! rather than re-constructing recognizers per call.
//!
//! Construction is opt-in — a `None` section simply means "no such
//! recognizer is available." Runs that request a kind whose section
//! is `None` fail with a validation error at engine assembly.
//!
//! [`Detection`]: super::Detection

use std::sync::Arc;

use nvisy_core::{Error, Result};

use super::llm::{LlmDetection, LlmRecognizer};
use super::ner::{NerDetection, NerRecognizer};
use super::pattern::{PatternDetection, PatternRecognizer};
use super::recognizer::RecognizerKind;

/// Registry of pre-built recognizers, addressed by [`RecognizerKind`].
///
/// Each slot is `Option<Arc<_>>` because the corresponding
/// `[recognizer.*]` config section is itself optional — operators
/// only configure the recognizers they need.
#[derive(Default, Clone)]
pub struct Recognizers {
    /// Pre-built LLM recognizer (when `[recognizer.llm]` is set).
    pub llm: Option<Arc<LlmRecognizer>>,
    /// Pre-built NER recognizer (when `[recognizer.ner]` is set).
    pub ner: Option<Arc<NerRecognizer>>,
    /// Pre-built pattern recognizer (when `[recognizer.pattern]` is set).
    pub pattern: Option<Arc<PatternRecognizer>>,
}

/// Configuration for the [`Recognizers`] registry.
///
/// Each field maps to a `[recognizer.*]` section in `Nvisy.toml`.
/// `None` opts the recognizer out entirely.
#[derive(Debug, Clone, Default, PartialEq)]
#[derive(serde::Serialize, serde::Deserialize)]
pub struct DetectionSection {
    /// `[recognizer.llm]` — LLM-backed recognizer config bundle.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub llm: Option<LlmDetection>,
    /// `[recognizer.ner]` — NER recognizer config bundle.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ner: Option<NerDetection>,
    /// `[recognizer.pattern]` — pattern recognizer config bundle.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pattern: Option<PatternDetection>,
}

impl DetectionSection {
    /// `true` when every section is `None`.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.llm.is_none() && self.ner.is_none() && self.pattern.is_none()
    }
}

impl Recognizers {
    /// Build the registry once from a [`DetectionSection`].
    ///
    /// Each opted-in section drives one recognizer construction.
    /// Construction is eager — model loads, HTTP-client setup, and
    /// regex compilation all happen here so per-run dispatch is
    /// cheap. Async because some NER backends initialise transports
    /// on first use.
    ///
    /// # Errors
    ///
    /// Returns the first construction error encountered (NER backend
    /// init failure, LLM provider misconfiguration). Pattern
    /// construction is infallible.
    pub async fn from_config(cfg: &DetectionSection) -> Result<Self> {
        let llm = cfg
            .llm
            .as_ref()
            .filter(|c| c.enabled)
            .map(|c| LlmRecognizer::new(c.clone()).map(Arc::new))
            .transpose()?;
        let ner = match cfg.ner.as_ref().filter(|c| c.enabled) {
            Some(c) => Some(Arc::new(NerRecognizer::from_config(c).await?)),
            None => None,
        };
        let pattern = cfg
            .pattern
            .as_ref()
            .filter(|c| c.enabled)
            .map(|c| Arc::new(PatternRecognizer::from_config(c)));
        Ok(Self { llm, ner, pattern })
    }

    /// `true` when no recognizers are configured.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.llm.is_none() && self.ner.is_none() && self.pattern.is_none()
    }

    /// Confirm `kind` is configured, returning a validation error
    /// otherwise. Used by [`Detection::into_engine`] before assembly.
    ///
    /// [`Detection::into_engine`]: super::Detection::into_engine
    pub fn require(&self, kind: RecognizerKind) -> Result<()> {
        let present = match kind {
            RecognizerKind::Llm => self.llm.is_some(),
            RecognizerKind::Ner => self.ner.is_some(),
            RecognizerKind::Pattern => self.pattern.is_some(),
        };
        if present {
            Ok(())
        } else {
            Err(Error::validation(
                format!(
                    "workflow requires recognizer `{kind:?}` but no \
                     `[recognizer.{}]` section is configured",
                    section_name(kind),
                ),
                "recognizers",
            ))
        }
    }
}

fn section_name(kind: RecognizerKind) -> &'static str {
    match kind {
        RecognizerKind::Llm => "llm",
        RecognizerKind::Ner => "ner",
        RecognizerKind::Pattern => "pattern",
    }
}
