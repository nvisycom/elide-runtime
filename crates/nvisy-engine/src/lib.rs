#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

//! The runtime adapter over [`elide`]. Engine is what makes elide a
//! long-running, multi-tenant, multi-document, two-phase redaction
//! service:
//!
//! - [`analyzer`] compiles a request's recognition plan into an
//!   [`elide::Analyzer`] (placeholder; lands as the detection
//!   surface grows).
//! - [`anonymizer`] compiles a request's [`nvisy_core::policy::Policy`]
//!   set into an [`elide::Anonymizer`] per modality, stamping each
//!   resolved decision with the policy + rule provenance the audit
//!   trail requires.
//! - [`registry`] is the multi-tenant, actor-scoped storage
//!   primitives ([`fjall`] keyspaces keyed by
//!   `[actor_id | resource_id]`).

pub mod analyzer;
pub mod anonymizer;
pub mod registry;

pub mod contexts;
pub mod policies;
pub mod runs;
