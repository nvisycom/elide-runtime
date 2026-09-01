#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

//! Layers on top of the [elide] toolkit. This crate wires elide's
//! per-modality analyzers, anonymizers, and orchestrator into a
//! stateless per-request document pipeline, driven by its own
//! request schemas ([`RequestContext`], [`file`](mod@file)) and the
//! governance schema from [elide-governance].
//!
//! [elide]: https://github.com/nvisycom/elide
//! [elide-governance]: https://docs.rs/elide-governance
//!
//! ## Architecture
//!
//! Stateless redaction pipeline over [`elide`]. Bytes (or a
//! caller-owned path) go in, a detection report or redacted
//! bytes come out. No persistence, no HTTP, no long-running
//! background tasks. Hosts (a SaaS API, a Tauri app, a CLI, a
//! language SDK) embed this crate and layer whatever workflow,
//! storage, and multi-tenancy they need on top.
//!
//! - [`Engine`]: the entry point. Bundles the codec registry,
//!   the deployment's NER / LLM lineups, the shared
//!   [`KeyProvider`] for keyed operators (HMAC, AES), and the
//!   per-request orchestrator builder.
//! - [`Engine::analyze`] and [`Engine::anonymize`] compile and
//!   run the full [`elide::Orchestrator`] against one document.
//!   Both take the request's `policies` alongside the document;
//!   each policy carries its own [`LabelScope`]s inline via
//!   [`PolicyDefinition::scopes`].
//! - [`Audit`] carries the analyze → anonymize handoff: the
//!   modality-tagged entity groups plus a [`DocumentContext`] with
//!   the request's asserted scope and correlation id.
//!
//! Ready-to-run policy sets for common regulatory postures
//! (HIPAA §164.514, GDPR Article 9, PCI DSS, CCPA / CPRA)
//! live in the sibling `elide-template` crate, re-exported here
//! as [`template`]. Each template carries a single
//! `PolicyDefinition` (with inline [`LabelScope`]s) that a caller
//! hands to [`Engine::analyze`] / [`Engine::anonymize`] as a
//! one-element slice.
//!
//! [`LabelScope`]: elide_governance::LabelScope
//! [`PolicyDefinition::scopes`]: elide_governance::PolicyDefinition::scopes
//!
//! [`elide`]: elide

pub mod entity;
pub mod file;
mod pipeline;

#[doc(inline)]
pub use elide::codec::FormatRegistry;

/// The modality markers every typed call is generic over.
///
/// Reading an [`Audit`] means naming one:
/// `audit.report.entities::<Text>()`, `audit.review::<Image>(..)`.
///
/// All four are always available: the modalities compile in
/// unconditionally, and only their codecs are feature-gated.
pub mod modality {
    #[doc(inline)]
    pub use elide::modality::Modality;
    #[doc(inline)]
    pub use elide::modality::audio::Audio;
    #[doc(inline)]
    pub use elide::modality::image::Image;
    #[doc(inline)]
    pub use elide::modality::tabular::Tabular;
    #[doc(inline)]
    pub use elide::modality::text::Text;
}
#[doc(inline)]
pub use elide::primitive::{CountryCode, Languages, RasterMode};
#[doc(inline)]
pub use elide::recognition::{
    ModelUsage, RecognizerId, ScopeMetadata, TokenCounts, Usage, UsageReport,
};
#[doc(inline)]
pub use elide::redaction::operators::KeyProvider;
/// The two halves of what an analysis produced, re-exported so a
/// consumer can name what [`Analyzed`] hands it.
///
/// [`Report`] is the reference half — entities and their audit
/// trails, no content — and rides on [`Audit::report`].
/// [`ArtifactSet`] is the content half, the enrichment a pass
/// extracted, and rides on [`Analyzed::artifacts`]. Both are
/// public fields, so both types have to be nameable to write a
/// signature over them.
///
/// [`Audit::report`]: Audit::report
/// [`Analyzed::artifacts`]: Analyzed::artifacts
#[doc(inline)]
pub use elide::{ArtifactSet, Report};
pub use elide::{Error, ErrorKind, Result};

/// Rendering an [`Audit`] into a transport format.
///
/// Only what a caller needs to *export*: the traits and the table
/// selector. `elide_export`'s implementor-facing plumbing
/// (`write_rows`, `TableRows`) stays out — writing a new
/// `ExportCsv` impl means depending on that crate directly.
#[cfg(any(feature = "audit-csv", feature = "audit-json"))]
#[cfg_attr(docsrs, doc(cfg(any(feature = "audit-csv", feature = "audit-json"))))]
pub mod export {
    /// Whole-document export: blanket implemented for every
    /// [`Serialize`](serde::Serialize) type, so
    /// [`Audit`](crate::Audit) gains it from its own serialization.
    #[cfg(feature = "audit-json")]
    #[cfg_attr(docsrs, doc(cfg(feature = "audit-json")))]
    #[doc(inline)]
    pub use elide_export::ExportJson;
    /// Flat-table export: [`Audit`](crate::Audit) projects into one
    /// CSV table per [`Table`] variant.
    ///
    /// The `audit-zip` feature adds `ExportCsv::write_zip`, bundling
    /// every table into a single archive.
    #[cfg(feature = "audit-csv")]
    #[cfg_attr(docsrs, doc(cfg(feature = "audit-csv")))]
    #[doc(inline)]
    pub use elide_export::{ExportCsv, Table};
}

#[doc(inline)]
pub use elide_governance as policy;
#[doc(inline)]
pub use elide_provider::{
    AttachTo, Backend, CodecParams, Component, DocumentContext, Enrichers, KeyConfig, LlmBackend,
    LlmSource, NerBackend, OcrBackend, Provider, ProviderConfig, Recognizers, RequestContext,
    SttBackend,
};
#[doc(inline)]
pub use elide_template as template;

pub use self::pipeline::{
    Analyzed, Audit, Engine, RegisteredComponents, RegisteredEnricher, RegisteredRecognizer,
    Unhandled,
};
