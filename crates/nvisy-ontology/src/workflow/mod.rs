//! Graph data model for pipeline definitions.
//!
//! A pipeline is represented as a set of [`GraphNode`]s connected by
//! [`GraphEdge`]s, collected into a [`Graph`]. Each node carries shared
//! fields (`id`, `retry`, `timeout`) alongside a [`GraphNodeKind`] that
//! determines what the node does.

mod context;
mod detection;
mod extraction;
mod ingest;
mod policy;
mod refinement;
mod validate;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

pub use self::context::{GenerateContext, LoadContext, SaveContext};
pub use self::detection::{Detection, NerDetection, PatternDetection};
pub use self::extraction::{
    AudialExtraction, Extraction, TextExtraction, VisualExtraction,
};
pub use self::ingest::{
    CompressionAlgorithm, EncryptionAlgorithm, EncryptionConfig, ExportFile, ImportFile,
};
pub use self::policy::{
    BackoffStrategy, ConcurrencyPolicy, RetryPolicy, TimeoutBehavior, TimeoutPolicy,
};
pub use self::refinement::{
    CalibrationMap, Fusion, FusionStrategy, GroupingCriteria, Redaction, Validation,
};
use crate::Error;

/// The set of strongly-typed actions a pipeline node can perform.
///
/// Each variant maps to one or more [`Operation`] implementations.
/// Variants carry a dedicated configuration struct.
///
/// [`Operation`]: crate::operation::Operation
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "action", rename_all = "snake_case")]
#[non_exhaustive]
pub enum GraphNodeKind {
    /// Loads reference-data contexts required by downstream actions.
    LoadContext(LoadContext),
    /// Persists contexts produced during the pipeline run.
    SaveContext(SaveContext),
    /// Generates a new context from detection results and content data.
    GenerateContext(GenerateContext),

    /// Extracts structured text from content (visual, audial, text).
    Extraction(Extraction),
    /// Detects entities via NER and/or pattern matching.
    Detection(Detection),

    /// Merges and scores entities from multiple detection sources.
    Fusion(Fusion),
    /// Applies redaction instructions to produce output content.
    Redaction(Redaction),
    /// Verifies that redacted content does not leak original values.
    Validation(Validation),

    /// Imports content into the pipeline for processing.
    ImportFile(ImportFile),
    /// Exports processed content to a target destination.
    ExportFile(ExportFile),
}

impl std::fmt::Display for GraphNodeKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::LoadContext(_) => f.write_str("load_context"),
            Self::SaveContext(_) => f.write_str("save_context"),
            Self::GenerateContext(_) => f.write_str("generate_context"),
            Self::Extraction(_) => f.write_str("extraction"),
            Self::Detection(_) => f.write_str("detection"),
            Self::Fusion(_) => f.write_str("fusion"),
            Self::Redaction(_) => f.write_str("redaction"),
            Self::Validation(_) => f.write_str("validation"),
            Self::ImportFile(_) => f.write_str("import"),
            Self::ExportFile(_) => f.write_str("export"),
        }
    }
}

impl GraphNodeKind {
    /// Returns the pipeline phase for this node kind.
    ///
    /// | Phase | Actions                                 |
    /// |-------|-----------------------------------------|
    /// | 0     | ImportFile, LoadContext                  |
    /// | 1     | Extraction                              |
    /// | 2     | Detection                               |
    /// | 3     | Fusion                                  |
    /// | 4     | Redaction, GenerateContext               |
    /// | 5     | Validation                              |
    /// | 6     | ExportFile, SaveContext                  |
    #[must_use]
    pub fn phase(&self) -> u8 {
        match self {
            Self::ImportFile(_) | Self::LoadContext(_) => 0,
            Self::Extraction(_) => 1,
            Self::Detection(_) => 2,
            Self::Fusion(_) => 3,
            Self::Redaction(_) | Self::GenerateContext(_) => 4,
            Self::Validation(_) => 5,
            Self::ExportFile(_) | Self::SaveContext(_) => 6,
        }
    }

    /// Returns `true` for nodes that run before policy evaluation:
    /// import, context loading, extraction, detection, and fusion.
    #[must_use]
    pub fn is_pre_redaction(&self) -> bool {
        matches!(
            self,
            Self::ImportFile(_)
                | Self::LoadContext(_)
                | Self::Extraction(_)
                | Self::Detection(_)
                | Self::Fusion(_)
        )
    }

    /// Returns `true` for the policy evaluation phase:
    /// redaction and context generation.
    #[must_use]
    pub fn is_redaction(&self) -> bool {
        matches!(self, Self::Redaction(_) | Self::GenerateContext(_))
    }

    /// Returns `true` for nodes that run after policy evaluation:
    /// validation, export, and save-context. These are skipped in
    /// dry-run mode.
    #[must_use]
    pub fn is_post_redaction(&self) -> bool {
        matches!(
            self,
            Self::Validation(_) | Self::ExportFile(_) | Self::SaveContext(_)
        )
    }

    /// Validates action-specific configuration.
    pub fn validate(&self) -> Result<(), Error> {
        match self {
            Self::ImportFile(cfg) => validate_struct(cfg),
            Self::LoadContext(cfg) => validate_struct(cfg),
            Self::SaveContext(cfg) => validate_struct(cfg),
            Self::ExportFile(cfg) => validate_struct(cfg),
            Self::Detection(cfg) => cfg
                .validate()
                .map_err(|e| Error::new(e.to_string())),
            Self::Extraction(_)
            | Self::Fusion(_)
            | Self::Redaction(_)
            | Self::Validation(_)
            | Self::GenerateContext(_) => Ok(()),
        }
    }
}

/// A node in the pipeline graph.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GraphNode {
    /// Unique identifier for this node within the graph.
    pub id: Uuid,
    /// Optional retry policy for this node.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry: Option<RetryPolicy>,
    /// Optional timeout policy for this node.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<TimeoutPolicy>,
    /// Action-specific payload.
    #[serde(flatten)]
    pub kind: GraphNodeKind,
}

impl GraphNode {
    pub fn new(id: Uuid, kind: GraphNodeKind) -> Self {
        Self {
            id,
            retry: None,
            timeout: None,
            kind,
        }
    }

    #[must_use]
    pub fn retry(&self) -> Option<&RetryPolicy> {
        self.retry.as_ref()
    }

    #[must_use]
    pub fn timeout(&self) -> Option<&TimeoutPolicy> {
        self.timeout.as_ref()
    }
}

/// A directed edge connecting two nodes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct GraphEdge {
    pub source: Uuid,
    pub target: Uuid,
}

/// A complete pipeline graph: nodes and directed edges forming a DAG.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Graph {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
    /// Optional concurrency limit for parallel document execution.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub concurrency: Option<ConcurrencyPolicy>,
}

impl Graph {
    pub fn new(nodes: Vec<GraphNode>, edges: Vec<GraphEdge>) -> Self {
        Self {
            nodes,
            edges,
            concurrency: None,
        }
    }
}

fn validate_struct(v: &impl Validate) -> Result<(), Error> {
    v.validate().map_err(|e| Error::new(e.to_string()))
}
