#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

//! The runtime adapter over [`elide`]. Engine is what makes elide a
//! long-running, multi-tenant, multi-document, two-phase redaction
//! service:
//!
//! - **[`policy_compile`]** — compiles a [`nvisy_core::policy::Policy`]
//!   into an [`elide::Anonymizer`] at request time, stamping each
//!   resolved decision with the policy + rule provenance the audit
//!   trail requires.
//! - **[`registry`]** — multi-tenant, actor-scoped storage primitives
//!   ([`fjall`] keyspaces keyed by `[actor_id | resource_id]`).
//! - everything else (request types, orchestration, audit, override
//!   workflow) lands as it is rebuilt on top of elide.

pub mod policy_compile;
pub mod registry;
