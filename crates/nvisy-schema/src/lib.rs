#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

//! ## Reference
//!
//! Wire schema for the Nvisy HTTP API. Serialisation + JSON
//! schema types shared between the server, any SDK that talks
//! to the server, and consumers generating bindings from the
//! OpenAPI spec.
//!
//! - [`policy`]: governance documents. Re-exported from
//!   [`nvisy-policy`].
//! - [`plan`]: analyzer plans. `AnalyzerParams`, per-recognizer
//!   configuration, dedup pipeline, request scope.
//! - [`file`](mod@file): persisted file descriptor and the
//!   in-memory carrier for codec input.
//! - [`primitive`], [`entity`], [`modality`], [`annotation`]:
//!   the slice of `elide_core` the wire types are built on,
//!   re-exported so SDK callers don't need `elide-core` as a
//!   separate dep.
//!
//! [`nvisy-policy`]: https://docs.rs/nvisy-policy

pub use nvisy_policy as policy;

pub mod annotation;
pub mod entity;
pub mod file;
pub mod modality;
pub mod plan;
pub mod primitive;
