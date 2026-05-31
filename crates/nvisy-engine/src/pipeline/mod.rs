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
mod engine;
mod envelope;
mod orchestrator;
mod run;
mod runs;

pub use self::config::{EngineConfig, ResourceLimits, RuntimeConfig};
pub use self::engine::{Engine, EngineInput, EngineOutput};
pub use self::envelope::{AnyEnvelope, DocumentEnvelope};
pub use self::runs::{
    AnalyticsSnapshot, NodeSnapshot, NodeStatus, RunEntry, RunFilter, RunOutcome, RunSnapshot,
    RunStatus,
};
// Re-export the phase contract from `core/` so historical consumers
// of `nvisy_engine::pipeline::{Phase,...}` keep compiling while we
// migrate imports.
pub use crate::core::{ModalityKind, Phase, PhaseContext, PhaseInfo, PhaseTarget, Plan};
