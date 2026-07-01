//! Retention: schedule storage, active-file gate, sweeper.
//!
//! Three concerns, all keyed off files rather than runs, and all
//! consumed by the periodic sweeper that deletes artifacts whose
//! deadline has passed:
//!
//! - **schedule** — the retention schedule keyspace, storing one
//!   [`RetentionRecord`] per `(actor, file, scope)`. Written by
//!   the run lifecycle at start ([`Engine::start_run`]) and after
//!   apply ([`Engine::apply_run`]), read by the sweeper.
//! - **active refs** — reverse index `(actor, file, run) → ()`
//!   the sweeper checks before deleting an original-content file:
//!   any surviving row means at least one non-terminal run still
//!   references the file.
//! - **sweeper** — background task ([`Engine::start_sweeper`]) +
//!   on-demand entry ([`Engine::sweep_once`]) that walks the
//!   schedule and calls into [`crate::FileRegistry::delete_file`]
//!   for every due row.
//!
//! Symmetric with [`crate::keyspace`] and [`crate::runs`]: state
//! lives in fjall keyspaces on the shared [`RegistryHandle`];
//! external callers reach it through the [`Engine`] methods.
//!
//! [`Engine`]: crate::Engine
//! [`Engine::start_run`]: crate::Engine::start_run
//! [`Engine::apply_run`]: crate::Engine::apply_run
//! [`Engine::sweep_once`]: crate::Engine::sweep_once
//! [`Engine::start_sweeper`]: crate::Engine::start_sweeper
//! [`RegistryHandle`]: crate::registry::RegistryHandle

pub(crate) mod active_refs;
pub(crate) mod schedule;
pub(crate) mod sweeper;

pub use self::schedule::RetentionRecord;
pub use self::sweeper::{SweepReport, SweeperHandle};
