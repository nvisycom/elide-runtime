#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

//! ## Reference
//!
//! Deployment-side runtime plumbing for the Nvisy platform.
//! Sibling to [`nvisy-schema`] (the wire schema). This crate
//! holds the deployment-only surface the SDK caller doesn't
//! need:
//!
//! - [`llm`]: deployment-owned LLM recognizer lineup + provider
//!   credentials. Wraps `elide-llm::Provider`; pulled only when
//!   the deployment runs LLM recognition.
//! - [`ner`]: deployment-owned NER recognizer lineup. Same shape
//!   as [`llm`]; the wire toggles on/off, the deployment picks
//!   which backends actually run.
//! - The runtime error vocabulary: [`Error`], [`ErrorKind`],
//!   [`Result`]. Distinct from elide's own error type; the
//!   runtime adds request-scoped context and surface categories
//!   the toolkit doesn't model.
//!
//! SDK consumers should depend on `nvisy-schema` directly.
//!
//! [`nvisy-schema`]: https://docs.rs/nvisy-schema

mod error;

pub mod llm;
pub mod ner;

pub use self::error::{Error, ErrorKind, Result};
