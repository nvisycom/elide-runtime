#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

//! ## Reference
//!
//! Wire schema for the Nvisy HTTP API. Serialisation + JSON
//! schema types shared between the server, any SDK that talks
//! to the server, and consumers generating bindings from the
//! OpenAPI spec. Runtime plumbing (LLM provider clients,
//! engine internals) lives in [`nvisy-core`] on top of this
//! crate.
//!
//! This crate is an umbrella. `plan`, `file`, and the
//! `elide_core` slice (`primitive`, `entity`, `modality`) live
//! here directly; `policy` and `context` come from their peer
//! crates, re-exported so a single `nvisy-schema` dep still
//! gives an SDK caller the whole wire surface.
//!
//! - [`policy`]: governance documents. Re-exported from
//!   [`nvisy-policy`].
//! - [`context`]: reference data submitted to enrich detection.
//!   Re-exported from [`nvisy-context`].
//! - [`plan`]: analyzer plans. `AnalyzerParams`, per-recognizer
//!   configuration, dedup pipeline, request scope.
//! - [`file`](mod@file): persisted file descriptor and the
//!   in-memory carrier for codec input.
//! - [`primitive`], [`entity`], [`modality`], [`annotation`]:
//!   the slice of `elide_core` the wire types are built on,
//!   re-exported so SDK callers don't need `elide-core` as a
//!   separate dep.
//!
//! [`nvisy-core`]: https://docs.rs/nvisy-core
//! [`nvisy-policy`]: https://docs.rs/nvisy-policy
//! [`nvisy-context`]: https://docs.rs/nvisy-context

pub use nvisy_context as context;
pub use nvisy_policy as policy;

pub mod annotation;
pub mod entity;
pub mod file;
pub mod modality;
pub mod plan;
pub mod primitive;
