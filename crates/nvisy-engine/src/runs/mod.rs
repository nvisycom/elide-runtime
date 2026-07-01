//! Run orchestrator: drives the two-phase analyze + apply
//! lifecycle over the fjall registry.
//!
//! The public surface hangs off [`Engine`] as methods, all
//! keyed by `(actor_id, run_id)`:
//!
//! - [`start_run`] — submit a batch, mints a run id, fans the
//!   analyzer out, lands in [`RunState::AwaitingReview`].
//! - [`get_run`] / [`get_run_doc`] — read the run header / a
//!   per-doc body.
//! - [`list_runs`] — list every run for an actor.
//! - [`override_entity`] — reviewer flips a per-entity decision
//!   before apply.
//! - [`apply_run`] — fan the anonymizer out, lands in
//!   [`RunState::Applied`] or [`RunState::PartiallyApplied`].
//! - [`cancel_run`] — mark an in-flight or awaiting-review run
//!   [`RunState::Failed`] with `reason = "cancelled"`.
//! - [`delete_run`] — cascade-remove a run across all run
//!   keyspaces.
//!
//! Retention (the schedule, active-file gate, sweeper) lives in
//! [`crate::retention`]; the run lifecycle writes into that
//! module's keyspaces at [`start_run`] and [`apply_run`] but the
//! sweeper concern is independent of any single run.
//!
//! [`Engine`]: crate::Engine
//! [`RegistryHandle`]: crate::registry::RegistryHandle
//! [`start_run`]: crate::Engine::start_run
//! [`apply_run`]: crate::Engine::apply_run
//! [`cancel_run`]: crate::Engine::cancel_run
//! [`delete_run`]: crate::Engine::delete_run
//! [`get_run`]: crate::Engine::get_run
//! [`get_run_doc`]: crate::Engine::get_run_doc
//! [`list_runs`]: crate::Engine::list_runs
//! [`override_entity`]: crate::Engine::override_entity

mod filter;
mod input;
mod orchestrate;
pub(crate) mod persist;
mod state;

pub use self::input::{DocumentInput, StartBatch};
pub use self::state::{
    DocBody, EntityRecord, FailureReason, RecognizedGroup, ResourceRef, Run, RunDocState,
    RunDocument, RunState,
};
