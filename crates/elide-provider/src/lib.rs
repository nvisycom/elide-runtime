#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

//! Deployment-configured recognition and redaction providers.
//!
//! A deployment decides some things once: which NER model, which
//! LLM provider, which OCR and STT engines, and the cryptographic
//! key provider the `HmacHash` and `Encrypt` operators resolve
//! through. [`ProviderConfig`] is that set as one serializable
//! value, and [`ProviderConfig::build`] turns it into a
//! [`Provider`].
//!
//! A [`Provider`] then builds an [`Orchestrator`] per request,
//! because an orchestrator carries request data — the policies in
//! force, the caller's scope and key, a correlation id — that no
//! deployment-wide value could hold. Config is parsed once, an
//! orchestrator is constructed per request.
//!
//! This crate depends only on [`elide`] and the governance
//! vocabulary, never on the pipeline: it is the half that turns
//! configuration into elide runtime values, and knows nothing about
//! documents, audits, or reviewer decisions.
//!
//! # Secrets
//!
//! Backend credentials travel inside the backend configs, because
//! that is where elide's own provider types keep them: an LLM
//! recognizer names its model and its API key together. A
//! serialized [`ProviderConfig`] therefore contains credentials,
//! and belongs wherever the deployment already keeps secrets rather
//! than in version control.
//!
//! Cryptographic keys are not here at all. A key belongs to the
//! caller asking for redaction, not to the process serving them, so
//! it travels on a [`RequestContext`] with the request that needs
//! it. One provider then serves many callers, each with its own.
//!
//! [`Orchestrator`]: elide::Orchestrator

use std::sync::Arc;

use elide::codec::FormatRegistry;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

mod context;
mod key;
mod orchestrator;
mod override_set;
pub mod plan;
mod recognition;
mod redaction;
mod request;

pub use self::context::AuditContext;
pub use self::key::KeyConfig;
pub use self::override_set::{Override, Overrides};
pub use self::recognition::{
    AttachTo, AuthenticatedProvider, Backend, Component, LlmBackend, LlmConfig, LlmSource,
    NerBackend, NerConfig, OcrBackend, OcrConfig, SttBackend, SttConfig, UnauthenticatedProvider,
};
pub use self::request::RequestContext;

/// Everything a deployment decides once, at startup.
///
/// Every field defaults to empty, so a config naming nothing builds
/// a provider that runs the pattern recognizers elide ships and no
/// model-backed ones.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct ProviderConfig {
    /// The NER recognizer lineup.
    pub ner: NerConfig,
    /// The LLM recognizer lineup.
    pub llm: LlmConfig,
    /// The OCR enricher lineup.
    pub ocr: OcrConfig,
    /// The STT enricher lineup.
    pub stt: SttConfig,
}

impl ProviderConfig {
    /// Build the provider this config describes.
    #[must_use]
    pub fn build(self) -> Provider {
        Provider::from_parts(self.ner, self.llm, self.ocr, self.stt)
    }
}

/// A deployment's configuration, ready to build orchestrators from.
///
/// Cheap to clone: every field is behind an [`Arc`], so a host can
/// hand one to each worker rather than rebuilding it.
#[derive(Clone)]
pub struct Provider {
    pub(crate) formats: Arc<FormatRegistry>,
    pub(crate) ner: Arc<NerConfig>,
    pub(crate) llm: Arc<LlmConfig>,
    pub(crate) ocr: Arc<OcrConfig>,
    pub(crate) stt: Arc<SttConfig>,
}

impl Provider {
    /// Assemble from already-built parts.
    ///
    /// Not the usual path: a deployment describes itself with a
    /// [`ProviderConfig`] and builds through it. This exists for a
    /// caller holding the pieces already.
    #[must_use]
    pub fn from_parts(ner: NerConfig, llm: LlmConfig, ocr: OcrConfig, stt: SttConfig) -> Self {
        Self {
            formats: Arc::new(FormatRegistry::with_builtin()),
            ner: Arc::new(ner),
            llm: Arc::new(llm),
            ocr: Arc::new(ocr),
            stt: Arc::new(stt),
        }
    }

    /// The codec registry documents are decoded through.
    #[must_use]
    pub fn formats(&self) -> &FormatRegistry {
        &self.formats
    }

    /// The NER lineup this provider was configured with.
    #[must_use]
    pub fn ner(&self) -> &NerConfig {
        &self.ner
    }

    /// The LLM lineup this provider was configured with.
    #[must_use]
    pub fn llm(&self) -> &LlmConfig {
        &self.llm
    }

    /// The OCR lineup this provider was configured with.
    #[must_use]
    pub fn ocr(&self) -> &OcrConfig {
        &self.ocr
    }

    /// The STT lineup this provider was configured with.
    #[must_use]
    pub fn stt(&self) -> &SttConfig {
        &self.stt
    }
}
