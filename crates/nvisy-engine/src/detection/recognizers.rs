//! [`Recognizers`]: the runtime-config-driven recognizer registry,
//! split per-modality.
//!
//! Built once at engine startup from the `[detection.*]` config
//! sections, holds each opted-in recognizer behind an `Arc`, and
//! is shared across every pipeline run. Per-run workflow
//! [`Detection`] nodes pick from this registry by [`RecognizerKind`]
//! rather than re-constructing recognizers per call.
//!
//! Construction is opt-in — a `None` section simply means "no such
//! recognizer is available." Runs that request a kind whose section
//! is `None` fail with a validation error at engine assembly.
//!
//! The registry is split per modality: [`TextRecognizers`] holds
//! text-side recognizers (llm/ner/pattern), [`ImageRecognizers`]
//! holds image-side recognizers (vlm). The engine's per-modality
//! [`Detect`] impl picks the matching slice.
//!
//! [`Detection`]: super::Detection
//! [`Detect`]: super::Detect

use std::sync::Arc;

use nvisy_agent::pipeline::{LlmNerPipeline, VlmPipeline};
use nvisy_core::{Error, Result};

use super::llm::{LlmDetection, build_pipeline as build_llm_pipeline};
use super::ner::{NerDetection, NerRecognizer};
use super::pattern::{PatternDetection, PatternRecognizer};
use super::recognizer::RecognizerKind;
use super::vlm::{VlmDetection, build_pipeline as build_vlm_pipeline};

/// Text-modality recognizer slots.
///
/// Each slot is `Option<Arc<_>>` because the corresponding
/// `[detection.*]` config section is itself optional — operators
/// only configure the recognizers they need.
#[derive(Default, Clone)]
pub struct TextRecognizers {
    /// Pre-built LLM pipeline (when `[detection.llm]` is set).
    pub llm: Option<Arc<LlmNerPipeline>>,
    /// Pre-built NER recognizer (when `[detection.ner]` is set).
    pub ner: Option<Arc<NerRecognizer>>,
    /// Pre-built pattern recognizer (when `[detection.pattern]` is set).
    pub pattern: Option<Arc<PatternRecognizer>>,
}

impl TextRecognizers {
    /// `true` when no text recognizer is configured.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.llm.is_none() && self.ner.is_none() && self.pattern.is_none()
    }
}

/// Image-modality recognizer slots.
#[derive(Default, Clone)]
pub struct ImageRecognizers {
    /// Pre-built VLM pipeline (when `[detection.vlm]` is set).
    pub vlm: Option<Arc<VlmPipeline>>,
}

impl ImageRecognizers {
    /// `true` when no image recognizer is configured.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.vlm.is_none()
    }
}

/// Top-level recognizer registry, split per modality.
#[derive(Default, Clone)]
pub struct Recognizers {
    /// Text-modality recognizers.
    pub text: TextRecognizers,
    /// Image-modality recognizers.
    pub image: ImageRecognizers,
}

/// Configuration for the [`Recognizers`] registry. Each field maps
/// to a `[detection.*]` section in `Nvisy.toml`. `None` opts the
/// recognizer out entirely.
#[derive(Debug, Clone, Default, PartialEq)]
#[derive(serde::Serialize, serde::Deserialize)]
pub struct DetectionSection {
    /// `[detection.llm]` — LLM-backed text recognizer config bundle.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub llm: Option<LlmDetection>,
    /// `[detection.ner]` — NER recognizer config bundle.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ner: Option<NerDetection>,
    /// `[detection.pattern]` — pattern recognizer config bundle.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pattern: Option<PatternDetection>,
    /// `[detection.vlm]` — VLM-backed image recognizer config bundle.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vlm: Option<VlmDetection>,
}

impl DetectionSection {
    /// `true` when every section is `None`.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.llm.is_none() && self.ner.is_none() && self.pattern.is_none() && self.vlm.is_none()
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
            .map(|c| build_llm_pipeline(c.clone()))
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
        let vlm = cfg
            .vlm
            .as_ref()
            .filter(|c| c.enabled)
            .map(|c| build_vlm_pipeline(c.clone()))
            .transpose()?;
        Ok(Self {
            text: TextRecognizers { llm, ner, pattern },
            image: ImageRecognizers { vlm },
        })
    }

    /// `true` when no recognizers are configured.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.text.is_empty() && self.image.is_empty()
    }

    /// Confirm `kind` is configured, returning a validation error
    /// otherwise. Used by [`Detection::into_engine`] before assembly.
    ///
    /// [`Detection::into_engine`]: super::Detection::into_engine
    pub fn require(&self, kind: RecognizerKind) -> Result<()> {
        let present = match kind {
            RecognizerKind::Llm => self.text.llm.is_some(),
            RecognizerKind::Ner => self.text.ner.is_some(),
            RecognizerKind::Pattern => self.text.pattern.is_some(),
            RecognizerKind::Vlm => self.image.vlm.is_some(),
        };
        if present {
            Ok(())
        } else {
            Err(Error::validation(
                format!(
                    "workflow requires recognizer `{kind:?}` but no \
                     `[detection.{}]` section is configured",
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
        RecognizerKind::Vlm => "vlm",
    }
}
