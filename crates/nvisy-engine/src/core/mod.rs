//! Engine core: the contract every phase programs against.
//!
//! `core/` houses the shapes flowing through every phase's `apply`
//! method. Phases (in `extraction/`, `detection/`, `deduplication/`,
//! `redaction/`, `validation/`) depend on `core/` only; they never
//! reach into `pipeline/` or each other. The execution layer
//! (`pipeline/`) consumes `core/` plus every phase module.
//!
//! # Contents
//!
//! - [`RunContext`] — per-run shared state (engines, policies,
//!   cancellation, dry-run flag).
//! - [`DocumentTree`] — the per-document tree of nodes phases walk;
//!   the [`NodeMut`] variant the walk yields drives the per-modality
//!   dispatch.
//! - [`DocumentView`] / [`ValueAt`] — the read-only view phases use to
//!   resolve a modality-typed location to its source string.
//! - [`Plan`] — the per-request bundle of per-phase configs phases
//!   read from `input.plan.X`.

mod context;
mod plan;
mod policy_store;
mod shared;
mod target;
mod tree;

pub use self::context::RunContext;
pub use self::plan::Plan;
pub(crate) use self::policy_store::Decision;
pub use self::policy_store::PolicyStore;
pub use self::shared::SharedData;
pub use self::target::{DocumentView, SharedHandle, ValueAt};
pub use self::tree::{AnyDocument, DocumentTree, NodeMut};
