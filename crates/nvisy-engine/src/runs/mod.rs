//! Run orchestrator: drives the two-phase analyze + apply
//! lifecycle over the fjall registry.
//!
//! Surface, all keyed by `(actor_id, run_id)`:
//!
//! - [`start`] — submit a batch, mints a run id, fans the
//!   analyzer out, lands in [`RunState::AwaitingReview`].
//! - [`get`] / [`get_doc`] — read the run header / a per-doc body.
//! - [`list`] — list every run for an actor.
//! - [`override_entity`] — reviewer flips a per-entity decision
//!   before apply.
//! - [`apply`] — fan the anonymizer out, lands in
//!   [`RunState::Applied`] or [`RunState::PartiallyApplied`].
//! - [`cancel`] — mark an in-flight or awaiting-review run
//!   [`RunState::Failed`] with `reason = "cancelled"`.
//! - [`delete`] — cascade-remove a run across all four
//!   keyspaces.
//!
//! Symmetric with [`crate::keyspace`]: all
//! engine state lives in fjall keyspaces on the shared
//! [`RegistryHandle`]; entry points read/write through the
//! [`EngineHandle`] that wraps it.
//!
//! [`EngineHandle`]: crate::EngineHandle
//! [`RegistryHandle`]: crate::registry::RegistryHandle

mod filter;
mod input;
mod orchestrate;
mod persist;
mod pipeline;
mod state;

pub use self::input::{DocumentInput, StartBatch};
pub use self::orchestrate::{apply, cancel, delete, get, get_doc, list, override_entity, start};
pub use self::state::{
    DocBody, EntityRecord, FailureReason, ModalityKind, ResourceRef, Run, RunDocState, RunDocument,
    RunState,
};
