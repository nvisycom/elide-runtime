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
//! - [`service`]: the runtime `Error` / `ErrorKind` vocabulary
//!   plus the `Healthcheck` composition trait.
//!
//! SDK consumers should depend on `nvisy-schema` directly.
//!
//! [`nvisy-schema`]: https://docs.rs/nvisy-schema

pub mod llm;
pub mod service;

pub use self::service::{Error, ErrorKind, Result};
