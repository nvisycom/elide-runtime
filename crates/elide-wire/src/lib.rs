#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

//! Layers on top of the [elide] toolkit. This crate adds the
//! `elide-runtime` wire schemas — plan (analyzer parameters) and
//! file (document envelope) — while primitives, entities,
//! modalities, and annotations stay in [`elide_core`].
//!
//! ## Reference
//!
//! Wire schemas for the `elide-runtime` HTTP API. Serialisation
//! + JSON schema types shared between the server, any SDK that
//! talks to the server, and consumers generating bindings from
//! the OpenAPI spec.
//!
//! - [`plan`]: analyzer plans. `AnalyzerParams`, per-recognizer
//!   configuration, dedup pipeline, request scope.
//! - [`file`](mod@file): persisted file descriptor and the
//!   in-memory carrier for codec input.
//!
//! Governance documents (policies, rules, predicates, operators)
//! live in the sibling [`elide-governance`] crate. Consumers
//! reach it directly rather than through this crate's re-export.
//!
//! [elide]: https://github.com/nvisycom/elide
//! [`elide-governance`]: https://docs.rs/elide-governance

pub mod file;
pub mod plan;
