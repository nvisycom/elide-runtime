#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

//! The runtime adapter over [`elide`]. Engine is what makes elide a
//! long-running, multi-tenant, multi-document, two-phase redaction
//! service:
//!
//! - [`analyzer`] compiles a request's recognition plan into an
//!   [`elide::Analyzer`] per modality.
//! - [`anonymizer`] compiles a request's [`nvisy_core::policy::Policy`]
//!   set into an [`elide::Anonymizer`] per modality, stamping each
//!   resolved decision with the policy + rule provenance the audit
//!   trail requires.
//! - [`registry`] holds the multi-tenant, actor-scoped storage
//!   primitives ([`fjall`] keyspaces keyed by `[actor_id | …]`).
//! - [`keyspace`] hosts the user-owned resource CRUD —
//!   [`PolicyRegistry`], [`FileRegistry`], [`ContextRegistry`] —
//!   all extension traits on [`registry::RegistryHandle`].
//! - [`runs`] is the orchestrator over the engine-managed run
//!   lifecycle, with its persistence trait kept `pub(crate)` so
//!   external code can't write malformed runs.

mod engine;

pub mod analyzer;
pub mod anonymizer;
pub mod keyspace;
pub mod registry;
pub mod runs;

pub use self::engine::EngineHandle;
pub use self::keyspace::{ContextRegistry, FileDescriptor, FileRegistry, PolicyRegistry};
