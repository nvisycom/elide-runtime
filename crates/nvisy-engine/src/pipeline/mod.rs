//! Pipeline engine: configuration, execution, and run tracking.
//!
//! The pipeline executes a user-submitted [`EngineInput`] — a flat,
//! fixed-order plan of phases (extraction → detection → dedup →
//! redaction → validation). The [`Engine`] is a thin facade that
//! delegates actual execution to `Pipeline` (one per run).
//!
//! # Submodules
//!
//! - `config`: [`RuntimeConfig`] and per-subsystem sections.
//! - `policy`: per-phase + per-run policy types.
//! - `run`: per-run lifecycle (`Pipeline`).
//! - `orchestrator`: concurrent document processing through the plan.
//! - `runs`: in-memory run lifecycle tracking.

mod config;
mod default;
mod orchestrator;
mod policy;
mod run;
mod runs;

pub use self::config::{CacheConfig, EngineSection, ResourceLimits, RuntimeConfig};
pub use self::default::{Engine, EngineInput, EngineOutput};
pub use self::policy::{ConcurrencyPolicy, PhasePolicy, TimeoutBehavior, TimeoutPolicy};
pub use self::runs::{
    AnalyticsSnapshot, NodeSnapshot, NodeStatus, RunEntry, RunFilter, RunOutcome, RunSnapshot,
    RunStatus,
};
