//! DAG execution engine for nvisy pipelines.
//!
//! This crate compiles pipeline definitions into directed acyclic graphs (DAGs),
//! plans topologically-ordered execution, and runs nodes concurrently with
//! retry and timeout policies.

#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

pub mod compiler;
pub mod connections;
pub mod engine;
pub mod executor;
pub mod policies;
pub mod runs;

#[doc(hidden)]
pub mod prelude;
