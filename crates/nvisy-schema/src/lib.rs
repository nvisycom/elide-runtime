#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

//! Wire schema for the Nvisy HTTP API.
//!
//! Serialisation + JSON schema types shared between the server,
//! any SDK that talks to the server, and consumers generating
//! bindings from the OpenAPI spec. Runtime plumbing (LLM
//! provider clients, engine internals, healthcheck traits) lives
//! in [`nvisy-core`] on top of this crate.
//!
//! ## Modules
//!
//! - [`policy`]: governance documents. `Policy`, `Rule`,
//!   `RuleAction`, `Predicate`, retention rules, etc.
//! - [`context`]: reference data submitted to enrich detection.
//! - [`plan`]: analyzer plans. `AnalyzerParams`, per-recognizer
//!   configuration, dedup pipeline, request scope.
//! - [`file`](mod@file): persisted file descriptors and lineage.
//! - [`primitive`], [`entity`], [`modality`]: the slice of
//!   `elide_core` the wire types are built on, re-exported so
//!   SDK callers don't need `elide-core` as a separate dep.
//!
//! [`nvisy-core`]: https://docs.rs/nvisy-core

pub mod context;
pub mod entity;
pub mod file;
pub mod modality;
pub mod plan;
pub mod policy;
pub mod primitive;
