//! Pipeline engine: configuration, compilation, execution, and run tracking.
//!
//! The pipeline processes content through a directed acyclic graph (DAG)
//! of operations: typically import → detect → evaluate → redact → export.
//! Callers submit an [`EngineInput`] containing a [`Graph`](nvisy_ontology::workflow::Graph),
//! policies, and optional config overrides. The [`Engine`] compiles the graph
//! into an execution plan, schedules nodes via the DAG orchestrator, and
//! returns an [`EngineOutput`] with detection results, policy evaluations,
//! and audit records.
//!
//! # Submodules
//!
//! - `config`: [`RuntimeConfig`] and per-subsystem sections (OCR, LLM, STT, TTS).
//! - `plan`: compiles a [`Graph`](nvisy_ontology::workflow::Graph) into a topologically-sorted
//!   execution plan.
//! - `orchestrator`: spawns one tokio task per node, gated by watch-channel
//!   dependency signals and an optional concurrency semaphore.
//! - `executor`: dispatches each node to its [`Operation`](crate::operation::Operation),
//!   running the envelope receive → extract → call → apply → send loop.
//! - `runs`: in-memory run lifecycle tracking ([`RunSnapshot`], [`RunEntry`]).
//! - `analytics`: point-in-time aggregate metrics across all tracked runs.

pub(crate) mod cache;
mod config;
mod default;
mod executor;
mod orchestrator;
mod plan;
mod runs;

pub use self::config::{
    EngineSection, LlmSection, OcrSection, ResourceLimits, RuntimeConfig, SttSection, TtsSection,
};
pub use self::default::{Engine, EngineInput, EngineOutput};
pub use self::runs::{
    AnalyticsSnapshot, NodeSnapshot, NodeStatus, RunEntry, RunFilter, RunOutcome, RunSnapshot,
    RunStatus,
};
