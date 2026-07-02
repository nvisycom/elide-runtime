#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

//! Server-facing runtime plumbing for the Nvisy platform.
//!
//! Sibling to [`nvisy-schema`] (the wire schema). This crate holds
//! the deployment-only surface the SDK caller doesn't need:
//!
//! - [`llm`]: deployment-owned LLM recognizer lineup + provider
//!   credentials. Wraps `elide-llm::Provider`; pulled only when
//!   the deployment runs LLM recognition.
//! - [`ner`]: deployment-owned NER recognizer lineup. Same shape
//!   as [`llm`]; the wire toggles on/off, the deployment picks
//!   which backends actually run.
//! - [`health`]: transport-agnostic healthcheck vocabulary
//!   ([`Healthcheck`] trait, [`ComponentCheck`], [`ServiceStatus`]).
//! - The runtime error vocabulary: [`Error`], [`ErrorKind`],
//!   [`Result`]. Distinct from elide's own error type; the
//!   runtime adds request-scoped context and surface categories
//!   the toolkit doesn't model.
//!
//! SDK consumers should depend on `nvisy-schema` directly.
//!
//! [`nvisy-schema`]: https://docs.rs/nvisy-schema
//! [`Healthcheck`]: health::Healthcheck
//! [`ComponentCheck`]: health::ComponentCheck
//! [`ServiceStatus`]: health::ServiceStatus

mod error;

pub mod health;
pub mod llm;
pub mod ner;

pub use self::error::{Error, ErrorKind, Result};
