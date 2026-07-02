//! User-owned resource CRUD over the engine's fjall keyspaces.
//!
//! Three resource families, all keyed by `(actor_id, …)`:
//!
//! - [`mod@policy`] — versioned [`Policy`] blobs keyed by
//!   `(actor_id, policy_id, version)`. Immutable per
//!   `(id, version)`; "edit" by writing a new version.
//! - [`mod@context`] — versioned [`Context`] blobs, same shape.
//! - [`mod@file`] — opaque `(metadata, bytes)` pair keyed by
//!   `(actor_id, file_id)`. Metadata + content live in
//!   separate keyspaces so `list_files` is cheap; bytes are
//!   loaded on demand.
//!
//! Each submodule exposes a `pub` extension trait on
//! [`RegistryHandle`] (`PolicyRegistry`, `ContextRegistry`,
//! `FileRegistry`). Method names are namespaced
//! (`put_policy`, `get_file`, `list_contexts`, …) so traits
//! can be brought into scope together without collisions.
//!
//! **Why these three live together.** They're the resources
//! the user directly owns — clients freely create, read,
//! modify, and delete them; the engine has no opinion about
//! when they may be written. Contrast with engine-managed
//! state (the run lifecycle), which lives in
//! [`crate::runs`] behind a `pub(crate)` storage trait. The
//! orchestrator coordinates run writes with state-machine
//! transitions, so writing one directly from outside the
//! engine would corrupt the invariants — hence the visibility
//! split.
//!
//! [`Policy`]: nvisy_schema::policy::Policy
//! [`Context`]: nvisy_schema::context::Context
//! [`RegistryHandle`]: crate::registry::RegistryHandle

pub mod context;
pub mod file;
pub mod policy;

pub use self::context::ContextRegistry;
pub use self::file::{FileDescriptor, FileRegistry};
pub use self::policy::PolicyRegistry;
