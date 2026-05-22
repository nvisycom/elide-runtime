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

use derive_more::{Display, From};
use nvisy_ontology::Error;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

pub use self::context::{GenerateContext, LoadContext, SaveContext};
pub use self::detection::{
    Detection, DetectionParams, LlmDetection, NerDetection, PatternDetection, PatternFilter,
};
pub use self::extraction::{AudialExtraction, Extraction, TextExtraction, VisualExtraction};
pub use self::ingest::{
    CompressionAlgorithm, EncryptionAlgorithm, EncryptionConfig, ExportFile, ImportFile,
};
pub use self::policy::{
    BackoffStrategy, ConcurrencyPolicy, RetryPolicy, TimeoutBehavior, TimeoutPolicy,
};
pub use self::refinement::{
    CalibrationMap, ConflictResolution, Deduplication, DeduplicationStrategy, GroupingCriteria,
    Redaction, Validation,
};

/// The set of strongly-typed actions a pipeline node can perform.
///
/// Each variant maps to one or more `Operation` implementations in the
/// engine. Variants carry a dedicated configuration struct.
#[derive(Debug, Clone, PartialEq, Display, From)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum GraphNodeKind {
    /// Loads reference-data contexts required by downstream actions.
    #[display("load_context")]
    LoadContext(LoadContext),
    /// Persists contexts produced during the pipeline run.
    #[display("save_context")]
    SaveContext(SaveContext),
    /// Generates a new context from detection results and content data.
    #[display("generate_context")]
    GenerateContext(GenerateContext),

    /// Extracts structured text from content (visual, audial, text).
    #[display("extraction")]
    Extraction(Extraction),
    /// Detects entities via NER and/or pattern matching.
    ///
    /// Boxed because [`Detection`] embeds full LLM provider config
    /// (`AgentProvider` + `AgentConfig`); inlining would inflate
    /// every other [`GraphNodeKind`] variant.
    #[display("detection")]
    Detection(Box<Detection>),

    /// Merges and scores entities from multiple detection sources.
    #[display("deduplication")]
    Deduplication(Deduplication),
    /// Applies redaction instructions to produce output content.
    #[display("redaction")]
    Redaction(Redaction),
    /// Verifies that redacted content does not leak original values.
    #[display("validation")]
    Validation(Validation),

    /// Imports content into the pipeline for processing.
    #[display("import")]
    ImportFile(ImportFile),
    /// Exports processed content to a target destination.
    #[display("export")]
    ExportFile(ExportFile),
}

impl GraphNodeKind {
    /// Returns the pipeline phase for this node kind.
    ///
    /// | Phase | Actions                                 |
    /// |-------|-----------------------------------------|
    /// | 0     | ImportFile, LoadContext                  |
    /// | 1     | Extraction                              |
    /// | 2     | Detection                               |
    /// | 3     | Deduplication                            |
    /// | 4     | Redaction, GenerateContext               |
    /// | 5     | Validation                              |
    /// | 6     | ExportFile, SaveContext                  |
    #[must_use]
    pub fn phase(&self) -> u8 {
        match self {
            Self::ImportFile(_) | Self::LoadContext(_) => 0,
            Self::Extraction(_) => 1,
            Self::Detection(_) => 2,
            Self::Deduplication(_) => 3,
            Self::Redaction(_) | Self::GenerateContext(_) => 4,
            Self::Validation(_) => 5,
            Self::ExportFile(_) | Self::SaveContext(_) => 6,
        }
    }

    /// Returns `true` for nodes that run before policy evaluation:
    /// import, context loading, extraction, detection, and deduplication.
    #[must_use]
    pub fn is_pre_redaction(&self) -> bool {
        matches!(
            self,
            Self::ImportFile(_)
                | Self::LoadContext(_)
                | Self::Extraction(_)
                | Self::Detection(_)
                | Self::Deduplication(_)
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
            Self::Detection(cfg) => cfg.validate().map_err(|e| Error::new(e.to_string())),
            Self::Extraction(_)
            | Self::Deduplication(_)
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

#[cfg(test)]
mod tests {
    use super::*;

    fn import() -> GraphNodeKind {
        GraphNodeKind::ImportFile(ImportFile::default())
    }

    fn extraction() -> GraphNodeKind {
        GraphNodeKind::Extraction(Extraction::default())
    }

    fn detection() -> GraphNodeKind {
        GraphNodeKind::Detection(Box::default())
    }

    fn dedup() -> GraphNodeKind {
        GraphNodeKind::Deduplication(Deduplication::default())
    }

    fn redaction() -> GraphNodeKind {
        GraphNodeKind::Redaction(Redaction::default())
    }

    fn validation() -> GraphNodeKind {
        GraphNodeKind::Validation(Validation::default())
    }

    fn export() -> GraphNodeKind {
        GraphNodeKind::ExportFile(ExportFile::default())
    }

    #[test]
    fn phases() {
        assert_eq!(import().phase(), 0);
        assert_eq!(
            GraphNodeKind::LoadContext(LoadContext {
                context_ids: vec![Uuid::nil()]
            })
            .phase(),
            0
        );
        assert_eq!(extraction().phase(), 1);
        assert_eq!(detection().phase(), 2);
        assert_eq!(dedup().phase(), 3);
        assert_eq!(redaction().phase(), 4);
        assert_eq!(validation().phase(), 5);
        assert_eq!(export().phase(), 6);
        assert_eq!(
            GraphNodeKind::SaveContext(SaveContext {
                context_ids: vec![Uuid::nil()],
            })
            .phase(),
            6
        );
    }

    #[test]
    fn pre_redaction() {
        assert!(import().is_pre_redaction());
        assert!(extraction().is_pre_redaction());
        assert!(detection().is_pre_redaction());
        assert!(dedup().is_pre_redaction());
        assert!(!redaction().is_pre_redaction());
        assert!(!validation().is_pre_redaction());
        assert!(!export().is_pre_redaction());
    }

    #[test]
    fn is_redaction() {
        assert!(!import().is_redaction());
        assert!(redaction().is_redaction());
        assert!(GraphNodeKind::GenerateContext(GenerateContext::default()).is_redaction());
    }

    #[test]
    fn post_redaction() {
        assert!(!import().is_post_redaction());
        assert!(!redaction().is_post_redaction());
        assert!(validation().is_post_redaction());
        assert!(export().is_post_redaction());
        assert!(
            GraphNodeKind::SaveContext(SaveContext {
                context_ids: vec![Uuid::nil()],
            })
            .is_post_redaction()
        );
    }

    #[test]
    fn validate_rejects_empty_import() {
        assert!(import().validate().is_err());
    }
}
