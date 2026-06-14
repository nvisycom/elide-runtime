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
//! - [`RuntimeConfig`] + [`EngineConfig`] + [`ResourceLimits`] —
//!   engine-wide deployment configuration shared between the
//!   detection and redaction engines.
//! - [`DetectionContext`] / [`RedactionContext`] — per-pass
//!   execution contexts, each carrying only the engine resources
//!   its side actually consumes. Both implement [`PhaseContext`]
//!   for the shared surface modality-agnostic phases bound on.
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

mod config;
mod context;
mod health;
pub mod ingestion;
mod policy_store;
mod shared;
mod target;
mod tree;

pub use nvisy_core::extraction::TextAt;

pub use self::config::{EngineConfig, ResourceLimits, RuntimeConfig};
pub use self::context::{DetectionContext, PhaseContext, RedactionContext};
pub(crate) use self::context::{DetectionEngines, RedactionEngines};
pub use self::health::probe_all;
pub(crate) use self::policy_store::Decision;
pub use self::policy_store::PolicyStore;
pub use self::shared::SharedData;
pub use self::tree::{AnyTree, DocumentTree};
