//! Pipeline engine: configuration, execution, and run tracking.
//!
//! The pipeline processes content through a directed acyclic graph (DAG)
//! of operations — typically import → detect → evaluate → redact → export.
//! Callers submit an [`EngineInput`] containing a [`Graph`](crate::graph::Graph),
//! policies, and optional config overrides. The [`Engine`] compiles the graph
//! into an [`ExecutionPlan`](plan::ExecutionPlan), schedules nodes via the
//! DAG orchestrator, and returns an [`EngineOutput`] with detection results,
//! policy evaluations, and audit records.
//!
//! Key internal modules:
//!
//! - [`config`] — [`RuntimeConfig`] and per-subsystem sections (OCR, LLM, STT, TTS).
//! - [`plan`] — compiles a [`Graph`](crate::graph::Graph) into a topologically-sorted
//!   [`ExecutionPlan`](plan::ExecutionPlan).
//! - [`orchestrator`] — spawns concurrent tokio tasks per node, gated by
//!   watch-channel dependency signals and an optional concurrency semaphore.
//! - [`executor`] — dispatches each node to its [`Operation`](crate::operation::Operation),
//!   managing the envelope recv → extract → call → apply → send loop.
//! - [`runs`] — in-memory run lifecycle tracking ([`RunSnapshot`], [`RunSummary`]).
//! - [`analytics`] — aggregate metrics across all runs.

mod analytics;
mod config;
mod default;
mod executor;
mod orchestrator;
mod plan;
mod runs;

pub use self::analytics::AnalyticsSnapshot;
pub use self::config::{
    EngineSection, LlmSection, OcrSection, RuntimeConfig, SttSection, TtsSection,
};
pub use self::default::{Engine, EngineInput, EngineOutput};
pub use self::runs::{NodeSnapshot, NodeStatus, RunFilter, RunSnapshot, RunStatus, RunSummary};
