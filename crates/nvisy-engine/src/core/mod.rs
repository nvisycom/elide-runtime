//! Engine core: the contract every phase programs against.
//!
//! `core/` houses the surface a [`Phase<M>`] implementor sees and
//! the data shapes flowing through `Phase::run`. Phases (in
//! `extraction/`, `detection/`, `deduplication/`, `redaction/`,
//! `validation/`) depend on `core/` only; they never reach into
//! `pipeline/`, `envelope/`, or each other. The execution layer
//! (`pipeline/`) consumes `core/` plus every phase module.
//!
//! # Contents
//!
//! - [`Phase<M>`] — the per-document operation trait every phase
//!   implements.
//! - [`PhaseContext`], [`PhaseInfo`], [`ModalityKind`] — phase
//!   introspection + per-call shared run state.
//! - [`PhaseTarget`] — the narrow view a phase mutates (doc +
//!   handle + run id + metadata + shared). Also hosts the per-modality
//!   [`ValueAt`] impls.
//! - [`Plan`] — the per-request bundle of per-phase configs phases
//!   read from `ctx.plan.X`.

mod context;
mod phase;
mod plan;
mod policy_store;
mod shared;
mod target;
mod tree;

pub use self::context::RunContext;
pub use self::phase::{ModalityKind, Phase, PhaseContext, PhaseInfo};
pub use self::plan::Plan;
pub(crate) use self::policy_store::Decision;
pub use self::policy_store::PolicyStore;
pub use self::shared::SharedData;
pub use self::target::{DocView, PhaseTarget, SharedHandle, ValueAt};
pub use self::tree::{AnyDocument, DocumentTree, NodeMut};
