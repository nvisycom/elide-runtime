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
//!   cancellation).
//! - [`SharedData`] — `Arc`-wrapped run-wide state (registry,
//!   codecs, policies).
//! - [`PolicyStore`] — per-modality policy storage + matching.
//! - [`DocumentTree<M>`] — the typed per-document carrier phases
//!   mutate.
//! - [`AnyTree`] — the modality-erased tree at the import boundary;
//!   the orchestrator matches once and dispatches into typed
//!   pipelines.
//! - [`TextAt`] / [`DataAt`] — implemented on [`DocumentTree<M>`]
//!   so phases that need to resolve a location pass `&tree`
//!   directly.
//! - [`probe_all`] — concurrent [`Healthcheck`] composition helper.
//!
//! [`DataAt`]: nvisy_core::extraction::DataAt
//! [`Healthcheck`]: nvisy_core::health::Healthcheck

mod context;
mod health;
mod policy_store;
mod shared;
mod target;
mod tree;

pub use nvisy_core::extraction::TextAt;

pub use self::context::RunContext;
pub(crate) use self::context::RunEngines;
pub use self::health::probe_all;
pub(crate) use self::policy_store::Decision;
pub use self::policy_store::PolicyStore;
pub use self::shared::SharedData;
pub use self::tree::{AnyTree, DocumentTree};
