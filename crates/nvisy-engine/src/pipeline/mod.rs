//! Pipeline engine: configuration, execution, and run tracking.
//!
//! The pipeline executes a user-submitted [`EngineInput`] — a flat,
//! fixed-order plan of phases (extraction → detection → dedup →
//! redaction → validation). The [`Engine`] is a thin facade that
//! delegates actual execution to per-run state in `run::Pipeline`.
//!
//! # Submodules
//!
//! - `config`: [`RuntimeConfig`] and per-subsystem sections.
//! - `document_pipeline`: per-document [`DocumentPipeline`] struct
//!   holding one concrete instance of each phase.
//! - `run`: per-run lifecycle (`run::Pipeline`).
//! - `orchestrator`: concurrent per-document fan-out.
//! - `runs`: in-memory run lifecycle tracking.

mod config;
mod document_pipeline;
mod engine;
mod orchestrator;
mod run;
mod runs;

pub use self::config::{EngineConfig, ResourceLimits, RuntimeConfig};
pub use self::engine::{Engine, EngineInput, EngineOutput};
pub use self::runs::{
    AnalyticsSnapshot, NodeSnapshot, NodeStatus, RunEntry, RunFilter, RunOutcome, RunSnapshot,
    RunStatus,
};
// Re-export the plan struct since several phase modules read
// `input.plan.X` directly. The `Phase` / `PhaseTarget` / `PhaseInfo`
// trio is gone — phases are concrete structs now.
pub use crate::core::Plan;
