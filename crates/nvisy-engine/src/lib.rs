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
//!   the deployment's NER / LLM lineups, and the per-request
//!   orchestrator builder.
//! - `analyze` / `apply` methods on [`Engine`] compile and run
//!   the full [`elide::Orchestrator`] against one document.
//!
//! [`elide`]: elide

mod analyzer;
mod anonymizer;
mod pipeline;
mod provider;

#[doc(inline)]
pub use elide::codec::FormatRegistry;
#[doc(inline)]
pub use elide::recognition::Scope;
#[doc(inline)]
pub use elide_core::{Error, ErrorKind, Result};
/// Authored redaction governance: policies, rules, predicates,
/// operators, retention.
#[doc(inline)]
pub use nvisy_schema::policy;
#[doc(inline)]
pub use nvisy_schema::{annotation, entity, file, modality, plan, primitive};

pub use self::analyzer::PatternGuardrails;
pub use self::pipeline::{
    AnalyzedDocument, AnonymizedDocument, Engine, EntityRecord, RecognizedGroup,
};
pub use self::provider::{llm, ner};
