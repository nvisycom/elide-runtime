//! Pipeline engine: configuration, compilation, execution, and run tracking.
//!
//! The pipeline processes content through a typed execution plan
//! derived from a user-submitted [`Graph`]. The [`Engine`] is a thin
//! facade that delegates actual execution to `Pipeline` (one per run).
//!
//! # Submodules
//!
//! - `config`: [`RuntimeConfig`] and per-subsystem sections.
//! - `plan`: compiles a [`Graph`] into a typed `ExecutionPlan`.
//! - `run`: per-run lifecycle (`Pipeline`).
//! - `orchestrator`: concurrent document processing through the plan.
//! - `runs`: in-memory run lifecycle tracking.
//!
//! [`Graph`]: crate::workflow::Graph

mod config;
mod default;
mod orchestrator;
mod plan;
mod run;
mod runs;

pub use self::config::{
    CacheConfig, EngineSection, LlmSection, OcrSection, ResourceLimits, RuntimeConfig, SttSection,
    TtsSection,
};
pub use self::default::{Engine, EngineInput, EngineOutput};
pub use self::runs::{
    AnalyticsSnapshot, NodeSnapshot, NodeStatus, RunEntry, RunFilter, RunOutcome, RunSnapshot,
    RunStatus,
};
