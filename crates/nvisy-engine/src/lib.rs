#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

pub mod graph;
pub mod operation;
pub mod pipeline;
pub mod provenance;

pub use self::graph::policy::{BackoffStrategy, RetryPolicy, TimeoutBehavior, TimeoutPolicy};
pub use self::graph::{Graph, GraphEdge, GraphNode, GraphNodeKind};
pub use self::pipeline::config::{
    EngineSection, LlmSection, OcrSection, RuntimeConfig, SttSection, TtsSection,
};
pub use self::pipeline::runs::{
    NodeSnapshot, NodeStatus, RunFilter, RunSnapshot, RunStatus, RunSummary,
};
pub use self::pipeline::{DefaultEngine, Engine, EngineInput, EngineOutput, Runs};
