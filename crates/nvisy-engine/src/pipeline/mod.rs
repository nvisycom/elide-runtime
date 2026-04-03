//! Pipeline engine: configuration, compilation, execution, and run tracking.
//!
//! The pipeline processes content through a directed acyclic graph (DAG)
//! of operations: typically import → detect → evaluate → redact → export.
//! Callers submit an [`EngineInput`] containing a [`Graph`], policies,
//! and optional config overrides. The [`Engine`] is a thin facade that
//! delegates actual execution to [`Pipeline`] (one per run).
//!
//! # Submodules
//!
//! - [`config`]: [`RuntimeConfig`] and per-subsystem sections.
//! - [`plan`]: compiles a [`Graph`] into a topologically-sorted plan.
//! - [`run`]: per-run lifecycle ([`Pipeline`]).
//! - [`orchestrator`]: spawns concurrent node tasks with dependency gating.
//! - [`executor`]: dispatches each node to its [`Operation`].
//! - [`transport`]: envelope fan-in, fan-out, and cloning between nodes.
//! - [`runs`]: in-memory run lifecycle tracking.
//!
//! [`Graph`]: nvisy_ontology::workflow::Graph
//! [`Pipeline`]: run::Pipeline
//! [`Operation`]: crate::operation::Operation

mod config;
mod default;
mod executor;
mod orchestrator;
mod plan;
mod run;
mod runs;
mod transport;

pub use self::config::{
    CacheConfig, EngineSection, LlmSection, OcrSection, ResourceLimits, RuntimeConfig, SttSection,
    TtsSection,
};
pub use self::default::{Engine, EngineInput, EngineOutput};
pub use self::runs::{
    AnalyticsSnapshot, NodeSnapshot, NodeStatus, RunEntry, RunFilter, RunOutcome, RunSnapshot,
    RunStatus,
};
