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
use super::nlp::{NlpDetection, NlpRecognizer};
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
    /// Pre-built NLP recognizer (when `[recognizer.nlp]` is set).
    pub nlp: Option<Arc<NlpRecognizer>>,
    /// Pre-built pattern recognizer (when `[recognizer.pattern]` is set).
    pub pattern: Option<Arc<PatternRecognizer>>,
}

/// Configuration for the [`Recognizers`] registry.
///
/// Each field maps to a `[recognizer.*]` section in `Nvisy.toml`.
/// `None` opts the recognizer out entirely.
#[derive(Debug, Clone, Default, PartialEq)]
#[derive(serde::Serialize, serde::Deserialize)]
pub struct RecognizerSection {
    /// `[recognizer.llm]` — LLM-backed recognizer config bundle.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub llm: Option<LlmDetection>,
    /// `[recognizer.nlp]` — NLP-engine recognizer config bundle.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nlp: Option<NlpDetection>,
    /// `[recognizer.pattern]` — pattern recognizer config bundle.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pattern: Option<PatternDetection>,
}

impl RecognizerSection {
    /// `true` when every section is `None`.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.llm.is_none() && self.nlp.is_none() && self.pattern.is_none()
    }
}

impl Recognizers {
    /// Build the registry once from a [`RecognizerSection`].
    ///
    /// Each opted-in section drives one recognizer construction.
    /// Construction is eager — model loads, HTTP-client setup, and
    /// regex compilation all happen here so per-run dispatch is
    /// cheap.
    ///
    /// # Errors
    ///
    /// Returns the first construction error encountered (NLP model
    /// load failure, LLM provider misconfiguration). Pattern
    /// construction is infallible.
    pub fn from_config(cfg: &RecognizerSection) -> Result<Self> {
        let llm = cfg
            .llm
            .as_ref()
            .filter(|c| c.enabled)
            .map(|c| LlmRecognizer::new(c.clone()).map(Arc::new))
            .transpose()?;
        let nlp = cfg
            .nlp
            .as_ref()
            .filter(|c| c.enabled)
            .map(|c| NlpRecognizer::from_config(c).map(Arc::new))
            .transpose()?;
        let pattern = cfg
            .pattern
            .as_ref()
            .filter(|c| c.enabled)
            .map(|c| Arc::new(PatternRecognizer::from_config(c)));
        Ok(Self { llm, nlp, pattern })
    }

    /// `true` when no recognizers are configured.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.llm.is_none() && self.nlp.is_none() && self.pattern.is_none()
    }

    /// Confirm `kind` is configured, returning a validation error
    /// otherwise. Used by [`Detection::into_engine`] before assembly.
    ///
    /// [`Detection::into_engine`]: super::Detection::into_engine
    pub fn require(&self, kind: RecognizerKind) -> Result<()> {
        let present = match kind {
            RecognizerKind::Llm => self.llm.is_some(),
            RecognizerKind::Nlp => self.nlp.is_some(),
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
        RecognizerKind::Nlp => "nlp",
        RecognizerKind::Pattern => "pattern",
    }
}
