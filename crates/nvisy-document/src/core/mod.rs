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
//! - [`DocumentTree<M>`] — the typed per-document carrier phases mutate.
//! - [`AnyTree`] — the modality-erased tree at the import boundary;
//!   the orchestrator matches once and dispatches into typed pipelines.
//! - [`TextAt`] / [`DataAt`] — implemented on [`DocumentTree<M>`]
//!   so phases that need to resolve a location pass `&tree` directly.
//! - [`Plan`] — the per-request bundle of per-phase configs phases
//!   read from `input.plan.X`.
//!
//! [`DataAt`]: nvisy_core::extraction::DataAt

mod context;
mod plan;
mod policy_store;
mod shared;
mod target;
mod tree;

pub use nvisy_core::extraction::TextAt;

pub use self::context::RunContext;
pub(crate) use self::context::RunEngines;
pub use self::plan::Plan;
pub(crate) use self::policy_store::Decision;
pub use self::policy_store::PolicyStore;
pub use self::shared::SharedData;
pub use self::tree::{AnyTree, DocumentTree};
