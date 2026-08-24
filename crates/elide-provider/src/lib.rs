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
//! force, the caller's scope, a correlation id — that no
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
//! Key material is the exception, and is deliberately not a field:
//! [`KeyConfig`] names its secrets by identifier, and the bytes
//! arrive separately in a [`Keyring`], so they have no path into a
//! serialized config at all.
//!
//! [`Orchestrator`]: elide::Orchestrator

use std::sync::Arc;

use elide::codec::FormatRegistry;
use elide::redaction::operators::KeyProvider;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

mod catalog;
mod context;
mod key;
mod orchestrator;
mod override_set;
pub mod plan;
mod recognition;
mod redaction;

pub use self::context::AuditContext;
pub use self::key::{KeyConfig, Keyring};
pub use self::override_set::{Override, Overrides};
pub use self::recognition::{
    AttachTo, AuthenticatedProvider, Backend, Component, LlmBackend, LlmConfig, LlmSource,
    NerBackend, NerConfig, OcrBackend, OcrConfig, SttBackend, SttConfig, UnauthenticatedProvider,
};

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
    /// Which key provider to build, if the deployment's policies
    /// use `HmacHash` or `Encrypt`.
    ///
    /// `None` leaves the provider without one: a policy naming
    /// either operator then fails at request-compile time, naming
    /// the policy and the operator, rather than redacting with some
    /// default key.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<KeyConfig>,
}

impl ProviderConfig {
    /// Build the provider this config describes.
    ///
    /// `keyring` supplies the secrets [`key`] names. Pass an empty
    /// one when no key provider is configured.
    ///
    /// A configured [`key`] whose secret the keyring does not hold,
    /// or a keyring with secrets no config names, is a mistake
    /// worth catching here rather than at the first request that
    /// needs a key: the second case means the deployment believes
    /// redaction is keyed when it is not.
    ///
    /// [`key`]: Self::key
    ///
    /// # Errors
    ///
    /// Returns [`Configuration`](elide::ErrorKind::Configuration)
    /// when the config and the keyring disagree.
    pub fn build(self, keyring: &Keyring) -> elide::Result<Provider> {
        let key_provider = match self.key.as_ref() {
            Some(config) => {
                let provider = config.build(keyring)?;
                reject_unused_secrets(config, keyring)?;
                Some(provider)
            }
            None if keyring.is_empty() => None,
            None => {
                return Err(elide::Error::new(
                    elide::ErrorKind::Configuration,
                    "a keyring was supplied but the config names no key provider",
                ));
            }
        };
        Ok(Provider::from_parts(
            self.ner,
            self.llm,
            self.ocr,
            self.stt,
            key_provider,
        ))
    }

    /// Build the provider, supplying the key provider directly.
    ///
    /// The escape hatch for a deployment whose keys are neither a
    /// static blob nor anything [`KeyConfig`] can name: implement
    /// [`KeyProvider`] and hand the instance over. [`key`] is
    /// ignored, since the caller has already answered the question
    /// it asks.
    ///
    /// [`key`]: Self::key
    #[must_use]
    pub fn build_with_key_provider(self, key_provider: Arc<dyn KeyProvider>) -> Provider {
        Provider::from_parts(self.ner, self.llm, self.ocr, self.stt, Some(key_provider))
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
    pub(crate) key_provider: Option<Arc<dyn KeyProvider>>,
}

impl Provider {
    /// Assemble from already-built parts.
    ///
    /// Not the usual path: a deployment describes itself with a
    /// [`ProviderConfig`] and builds through it. This exists for a
    /// caller holding the pieces already.
    #[must_use]
    pub fn from_parts(
        ner: NerConfig,
        llm: LlmConfig,
        ocr: OcrConfig,
        stt: SttConfig,
        key_provider: Option<Arc<dyn KeyProvider>>,
    ) -> Self {
        Self {
            formats: Arc::new(FormatRegistry::with_builtin()),
            ner: Arc::new(ner),
            llm: Arc::new(llm),
            ocr: Arc::new(ocr),
            stt: Arc::new(stt),
            key_provider,
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

/// Reject a keyring holding a secret `config` never asks for.
///
/// The mirror of the missing-secret check, and the one that catches
/// a typo: a deployment that supplies `redcation` for a config
/// naming `redaction` gets a clear failure at startup instead of a
/// provider quietly built without the key it meant to use.
fn reject_unused_secrets(config: &KeyConfig, keyring: &Keyring) -> elide::Result<()> {
    let named: Vec<&str> = config.secrets().collect();
    let mut unused: Vec<&str> = keyring
        .names()
        .filter(|name| !named.contains(name))
        .collect();
    if unused.is_empty() {
        return Ok(());
    }
    unused.sort_unstable();
    Err(elide::Error::new(
        elide::ErrorKind::Configuration,
        format!(
            "the keyring holds secrets the config never names: {}",
            unused.join(", ")
        ),
    ))
}
