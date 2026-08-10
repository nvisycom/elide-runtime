#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

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
//!   each policy carries its own [`LabelGroup`]s inline via
//!   [`PolicyDefinition::groups`].
//! - [`Audit`] carries the analyze → anonymize handoff: the
//!   modality-tagged entity groups plus an [`AuditContext`] with
//!   the request's asserted scope and correlation id.
//!
//! Ready-to-run policy sets for common regulatory postures
//! (HIPAA §164.514, GDPR Article 9, PCI DSS, CCPA / CPRA)
//! live in the sibling `nvisy-template` crate, re-exported here
//! as [`template`]. Each template carries a single
//! `PolicyDefinition` (with inline [`LabelGroup`]s) that a caller
//! hands to [`Engine::analyze`] / [`Engine::anonymize`] as a
//! one-element slice.
//!
//! [`LabelGroup`]: nvisy_schema::policy::LabelGroup
//! [`PolicyDefinition::groups`]: nvisy_schema::policy::PolicyDefinition::groups
//!
//! [`elide`]: elide

mod analyzer;
mod anonymizer;
pub mod entity;
mod pipeline;
pub mod provider;

#[doc(inline)]
pub use elide::redaction::operators::KeyProvider;
#[doc(inline)]
pub use elide_core::primitive::OcrMode;
pub use elide_core::{Error, ErrorKind, Result};
#[doc(inline)]
pub use nvisy_schema::file::{Document, FileMetadata};
/// Authored redaction governance: policies, rules, predicates,
/// operators.
#[doc(inline)]
pub use nvisy_schema::policy;
/// Ready-to-run policy templates for common regulatory postures
/// (HIPAA §164.514, GDPR Article 9, PCI DSS, CCPA / CPRA).
#[doc(inline)]
pub use nvisy_template as template;

pub use self::entity::EntityGroup;
pub use self::pipeline::{Audit, AuditContext, Engine};
