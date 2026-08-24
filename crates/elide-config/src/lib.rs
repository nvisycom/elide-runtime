#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

//! Deployment configuration for an [`Engine`].
//!
//! An engine is assembled once at startup from things a deployment
//! owns: which NER model, which LLM provider, which OCR and STT
//! engines, and the cryptographic key provider the `HmacHash` and
//! `Encrypt` operators resolve through. [`EngineConfig`] is that
//! set as one serializable value, and [`EngineConfig::build`] turns
//! it into a running engine.
//!
//! This lives apart from `elide-pipeline` so the engine crate holds
//! the *running* engine and nothing about where its configuration
//! came from. A host reads a file, an environment variable, or an
//! encrypted row in its own database, fills in an `EngineConfig`,
//! and builds. The pipeline never learns which.
//!
//! # Secrets
//!
//! Backend credentials travel inside the backend configs, because
//! that is where elide's own provider types keep them: an LLM
//! recognizer names its model and its API key together. A
//! serialized `EngineConfig` therefore contains credentials, and
//! belongs wherever the deployment already keeps secrets rather
//! than in version control.
//!
//! Key material is the exception, and is deliberately not a field:
//! [`KeyConfig`] names its secrets by identifier, and the bytes
//! arrive separately in a [`Keyring`] the deployment fills from
//! wherever it keeps them. So they have no path into a serialized
//! config at all, and a provider needing two secrets rather than
//! one changes nothing about how they travel.
//!
//! [`Engine`]: elide_pipeline::Engine

use std::sync::Arc;

use elide::redaction::operators::KeyProvider;
use elide_pipeline::Engine;
use elide_pipeline::recognition::{LlmConfig, NerConfig, OcrConfig, SttConfig};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

mod key;

pub use self::key::{KeyConfig, Keyring};

/// Everything a deployment decides once, at startup.
///
/// Every field defaults to empty, so a config naming nothing at all
/// builds an engine that runs the pattern recognizers elide ships
/// and no model-backed ones.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct EngineConfig {
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
    /// `None` leaves the engine without one: a policy naming either
    /// operator then fails at request-compile time, naming the
    /// policy and the operator, rather than redacting with some
    /// default key.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<KeyConfig>,
}

impl EngineConfig {
    /// Build the engine this config describes.
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
    pub fn build(self, keyring: &Keyring) -> elide::Result<Engine> {
        let key_provider = match (self.key.as_ref(), keyring.is_empty()) {
            (Some(config), _) => Some(config.build(keyring)?),
            (None, true) => None,
            (None, false) => {
                return Err(elide::Error::new(
                    elide::ErrorKind::Configuration,
                    "a keyring was supplied but the engine config names no key provider",
                ));
            }
        };
        Ok(Engine::from_parts(
            self.ner,
            self.llm,
            self.ocr,
            self.stt,
            key_provider,
        ))
    }

    /// Build the engine, supplying the key provider directly.
    ///
    /// The escape hatch for a deployment whose keys are neither a
    /// static blob nor anything [`KeyConfig`] can name: implement
    /// [`KeyProvider`] and hand the instance over. [`key`] is
    /// ignored, since the caller has already answered the question
    /// it asks.
    ///
    /// [`key`]: Self::key
    #[must_use]
    pub fn build_with_key_provider(self, key_provider: Arc<dyn KeyProvider>) -> Engine {
        Engine::from_parts(self.ner, self.llm, self.ocr, self.stt, Some(key_provider))
    }
}
