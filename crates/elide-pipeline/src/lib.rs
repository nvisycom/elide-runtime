#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

//! Layers on top of the [elide] toolkit. This crate wires elide's
//! per-modality analyzers, anonymizers, and orchestrator into a
//! stateless per-request document pipeline driven by the wire
//! schemas from [`elide_wire`] and the governance schema from
//! [elide-governance].
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
mod pipeline;
pub mod provider;

#[doc(inline)]
pub use elide::codec::FormatRegistry;
#[doc(inline)]
pub use elide::redaction::operators::KeyProvider;
#[doc(inline)]
pub use elide_core::primitive::RasterMode;
pub use elide_core::{Error, ErrorKind, Result};
/// Authored redaction governance: policies, rules, predicates,
/// operators.
#[doc(inline)]
pub use elide_governance as policy;
/// Ready-to-run policy templates for common regulatory postures
/// (HIPAA §164.514, GDPR Article 9, PCI DSS, CCPA / CPRA).
#[doc(inline)]
pub use elide_template as template;
#[doc(inline)]
pub use elide_wire::file::{Document, FileMetadata};
/// Authored recognition plan: `AnalyzerParams`, caller-inlined
/// pattern extras, scope, region annotations.
#[doc(inline)]
pub use elide_wire::plan;

pub use self::entity::EntityGroup;
pub use self::pipeline::{Audit, AuditContext, Engine, RegisteredRecognizer};
