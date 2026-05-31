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
//! - `run`: per-run lifecycle (`Pipeline`).
//! - `orchestrator`: concurrent document processing through the plan.
//! - `phase`: the `Phase<M>` trait every per-document step implements.
//! - `runs`: in-memory run lifecycle tracking.

mod config;
mod default;
mod orchestrator;
mod phase;
mod plan;
mod run;
mod runs;
mod target;

pub use self::config::{EngineConfig, ResourceLimits, RuntimeConfig};
pub use self::default::{Engine, EngineInput, EngineOutput};
pub use self::phase::{ModalityKind, Phase, PhaseContext, PhaseInfo};
pub use self::plan::Plan;
pub use self::runs::{
    AnalyticsSnapshot, NodeSnapshot, NodeStatus, RunEntry, RunFilter, RunOutcome, RunSnapshot,
    RunStatus,
};
pub use self::target::PhaseTarget;
