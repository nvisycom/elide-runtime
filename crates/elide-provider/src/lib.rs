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

mod orchestrator;
mod recognition;
mod redaction;

pub use self::orchestrator::{
    KeyConfig, Override, Overrides, Provider, ProviderConfig, RequestContext, RequestScope,
    scope_metadata_is_empty,
};
pub use self::recognition::{
    AttachTo, AuthenticatedProvider, Backend, Component, Enrichers, LlmBackend, LlmSource,
    NerBackend, OcrBackend, Recognizers, SttBackend, UnauthenticatedProvider,
};
