#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

//! The runtime adapter over [`elide`]. Engine is what makes elide a
//! long-running, multi-tenant, multi-document, two-phase redaction
//! service:
//!
//! - [`Engine`] bundles persistence + codec registry + the
//!   per-request orchestrator constructor (analyze and apply on
//!   one document).
//! - [`registry`] holds the multi-tenant, actor-scoped storage
//!   primitives ([`fjall`] keyspaces keyed by `[actor_id | …]`).
//! - [`keyspace`] hosts the user-owned resource CRUD —
//!   [`PolicyRegistry`], [`FileRegistry`], [`ContextRegistry`] —
//!   all extension traits on [`registry::RegistryHandle`].
//! - [`runs`] is the multi-doc batched run orchestrator on top of
//!   the per-document verbs, with its persistence trait kept
//!   `pub(crate)` so external code can't write malformed runs.
//! - [`retention`] holds the retention schedule, the active-file
//!   reverse index gating the sweeper, and the sweeper itself
//!   ([`Engine::sweep_once`] / [`Engine::start_sweeper`]).

mod engine;
mod llm;

pub mod keyspace;
pub mod registry;
pub mod retention;
pub mod runs;

pub use self::engine::{ApplyOutcome, Engine};
pub use self::keyspace::{ContextRegistry, FileDescriptor, FileRegistry, PolicyRegistry};
