//! Multi-tenant registry over fjall.
//!
//! Three keyspace shapes, all actor-scoped:
//!
//! - **policies** + **contexts** keyspaces hold [`Policy`] and
//!   [`Context`] resources versioned in place. Lookup by
//!   `(actor_id, resource_id, version)`; latest-version lookup
//!   range-scans by `(actor_id, resource_id)`.
//! - **run_headers** holds short metadata blobs for each run (state,
//!   refs to policies/contexts, timestamps). Lookup by
//!   `(actor_id, run_id)`.
//! - **run_docs** holds the per-document body for each run
//!   (recognized entities + reviewer overrides + post-apply
//!   redacted bytes/audit). Lookup by `(actor_id, run_id, doc_id)`.
//!
//! [`Policy`]: nvisy_core::policy::Policy
//! [`Context`]: nvisy_core::context::Context

mod fjall_ext;
mod handle;
mod key;
mod paged;

pub(crate) use self::fjall_ext::{blocking, not_found};
pub use self::handle::RegistryHandle;
pub(crate) use self::key::{CompositeKey, TripleKey, VersionedKey};
pub(crate) use self::paged::PagedResult;
