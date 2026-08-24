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

mod orchestrator;
mod recognition;
mod redaction;

pub use self::orchestrator::{
    KeyConfig, Override, Overrides, RequestContext, RequestScope, scope_metadata_is_empty,
};
pub use self::recognition::{
    AttachTo, AuthenticatedProvider, Backend, Component, Enrichers, LlmBackend, LlmSource,
    NerBackend, OcrBackend, Recognizers, SttBackend, UnauthenticatedProvider,
};

/// Everything a deployment decides once, at startup.
///
/// Every field defaults to empty, so a config naming nothing builds
/// a provider that runs the pattern recognizers elide ships and no
/// model-backed ones.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct ProviderConfig {
    /// The recognizer lineups: which components find entities.
    pub recognizers: Recognizers,
    /// The enricher lineups: which components produce the context
    /// recognizers read.
    pub enrichers: Enrichers,
}

impl ProviderConfig {
    /// Build the provider this config describes.
    #[must_use]
    pub fn build(self) -> Provider {
        Provider::from_parts(self.recognizers, self.enrichers)
    }
}

/// A deployment's configuration, ready to build orchestrators from.
///
/// Cheap to clone: one [`Arc`] around the whole configuration, so a
/// host hands a clone to each worker rather than rebuilding it, and
/// a clone costs one refcount rather than one per field.
#[derive(Debug, Clone)]
pub struct Provider {
    inner: Arc<ProviderInner>,
}

/// The configuration a [`Provider`] shares between its clones.
///
/// Behind one [`Arc`] rather than an `Arc` per field: these are
/// decided together at startup, read together on every request, and
/// never change independently, so they are one value.
#[derive(Debug)]
struct ProviderInner {
    /// The codec registry documents decode through.
    formats: FormatRegistry,
    /// The recognizer lineups.
    recognizers: Recognizers,
    /// The enricher lineups.
    enrichers: Enrichers,
}

impl Provider {
    /// Assemble from already-built parts.
    ///
    /// Not the usual path: a deployment describes itself with a
    /// [`ProviderConfig`] and builds through it. This exists for a
    /// caller holding the pieces already.
    #[must_use]
    pub fn from_parts(recognizers: Recognizers, enrichers: Enrichers) -> Self {
        Self {
            inner: Arc::new(ProviderInner {
                formats: FormatRegistry::with_builtin(),
                recognizers,
                enrichers,
            }),
        }
    }

    /// The codec registry documents are decoded through.
    #[must_use]
    pub fn formats(&self) -> &FormatRegistry {
        &self.inner.formats
    }

    /// The recognizer lineups this provider was configured with.
    #[must_use]
    pub fn recognizers(&self) -> &Recognizers {
        &self.inner.recognizers
    }

    /// The enricher lineups this provider was configured with.
    #[must_use]
    pub fn enrichers(&self) -> &Enrichers {
        &self.inner.enrichers
    }
}
