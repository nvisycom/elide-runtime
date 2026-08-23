#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

//! Layers on top of the [elide] toolkit. This crate wires elide's
//! per-modality analyzers, anonymizers, and orchestrator into a
//! stateless per-request document pipeline, driven by its own
//! request schemas ([`plan`], [`file`](mod@file)) and the
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
//!   modality-tagged entity groups plus an [`AuditContext`] with
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

mod analyzer;
mod anonymizer;
pub mod entity;
pub mod file;
mod pipeline;
pub mod plan;
pub mod provider;

#[doc(inline)]
pub use elide::codec::FormatRegistry;
/// The modality markers [`EntityGroup`]'s variants are generic
/// over. Re-exported because [`EntityRecord<M>`] cannot be named
/// without them: a caller matching on an [`Audit`]'s body needs
/// `EntityRecord<Text>` and its siblings by name.
///
/// All four are always available: the modalities compile in
/// unconditionally, and only their codecs are feature-gated.
///
/// [`EntityRecord<M>`]: crate::entity::EntityRecord
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
#[doc(inline)]
pub use elide::primitive::{CountryCode, Languages, RasterMode};
/// Per-component usage accounting, surfaced on [`Audit::usage`].
///
/// The whole chain is re-exported, not just the report: `entries`,
/// [`UsageReport::by_name`], and [`UsageReport::extend`] all traffic
/// in [`Usage`], whose `id` and `model` in turn expose
/// [`RecognizerId`] and [`ModelUsage`] / [`TokenCounts`]. Without
/// these a caller can reach a value off the audit but cannot name
/// its type.
///
/// [`Audit::usage`]: crate::Audit::usage
#[doc(inline)]
pub use elide::recognition::{
    ModelUsage, RecognizerId, ScopeMetadata, TokenCounts, Usage, UsageReport,
};
#[doc(inline)]
pub use elide::redaction::operators::KeyProvider;
pub use elide::{Error, ErrorKind, Result};
/// Authored redaction governance: policies, rules, predicates,
/// operators.
#[doc(inline)]
pub use elide_governance as policy;
/// Ready-to-run policy templates for common regulatory postures
/// (HIPAA §164.514, GDPR Article 9, PCI DSS, CCPA / CPRA).
#[doc(inline)]
pub use elide_template as template;

pub use self::entity::EntityGroup;
pub use self::file::{Document, FileMetadata};
pub use self::pipeline::{
    Audit, AuditContext, Engine, RegisteredComponents, RegisteredEnricher, RegisteredRecognizer,
};
