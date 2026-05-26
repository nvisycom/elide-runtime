//! Pipeline run lifecycle types and in-memory storage.
//!
//! A run progresses through [`RunStatus`] states: `Pending` → `Running` →
//! `Succeeded` | `PartialFailure` | `Failed` | `Cancelled`. Each node
//! within a run tracks its own [`NodeStatus`].
//!
//! Two projection types serve different API needs:
//!
//! - [`RunSnapshot`] — full detail including per-node snapshots and
//!   a type-safe [`RunOutcome`] (`GET /runs/{id}`).
//! - [`RunEntry`] — lightweight with node count only, no per-node
//!   detail (`GET /runs`).
//!
//! The [`state`] submodule contains the volatile in-memory storage
//! ([`RunState`]) backing all run queries and mutations.
//!
//! [`RunState`]: state::RunState

mod analytics;
pub(crate) mod state;

use nvisy_ontology::modality::Text;
use jiff::Timestamp;
use nvisy_ontology::provenance::Audit;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub use self::analytics::AnalyticsSnapshot;

/// Lifecycle status of a pipeline run (internal tracking tag).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    /// The run has been created but not yet started.
    Pending,
    /// The run is actively executing nodes.
    Running,
    /// All nodes completed without error.
    Succeeded,
    /// Some nodes succeeded while others failed.
    PartialFailure,
    /// All nodes failed.
    Failed,
    /// The run was cancelled by the caller.
    Cancelled,
}

/// Lifecycle status of a single node within a run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum NodeStatus {
    /// The node is waiting for upstream dependencies.
    Pending,
    /// The node is actively executing.
    Running,
    /// The node completed without error.
    Succeeded,
    /// The node failed.
    Failed,
}

/// Point-in-time snapshot of a single node within a run.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct NodeSnapshot {
    /// ID of the node.
    pub node_id: Uuid,
    /// Current status of this node.
    pub status: NodeStatus,
    /// Number of data items processed so far.
    pub items_processed: u64,
    /// Error message if the node failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Rich outcome of a pipeline run, carrying state-specific data.
///
/// `Succeeded` and `PartialFailure` include audit trails (populated
/// from the registry by [`Engine::get_run`]). `Failed` carries an
/// optional error message. `Pending` and `Running` have no extra data.
///
/// [`Engine::get_run`]: crate::pipeline::Engine::get_run
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum RunOutcome {
    /// The run has been created but not yet started.
    Pending,
    /// The run is actively executing nodes.
    Running,
    /// All nodes completed without error.
    Succeeded {
        /// When the run reached a terminal state.
        #[schemars(with = "String")]
        completed_at: Timestamp,
        /// Per-document audit trails.
        audits: Vec<Audit<Text>>,
        /// Total entities detected across all nodes.
        entities_detected: u64,
        /// Total redactions applied across all nodes.
        redactions_applied: u64,
    },
    /// Some nodes succeeded while others failed.
    PartialFailure {
        /// When the run reached a terminal state.
        #[schemars(with = "String")]
        completed_at: Timestamp,
        /// Per-document audit trails from successful nodes.
        audits: Vec<Audit<Text>>,
        /// Total entities detected across all nodes.
        entities_detected: u64,
        /// Total redactions applied across all nodes.
        redactions_applied: u64,
    },
    /// All nodes failed.
    Failed {
        /// When the run reached a terminal state.
        #[schemars(with = "String")]
        completed_at: Timestamp,
        /// Error description, if available.
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },
    /// The run was cancelled by the caller.
    Cancelled {
        /// When the run was cancelled.
        #[schemars(with = "String")]
        completed_at: Timestamp,
    },
}

/// Full point-in-time snapshot of a pipeline run.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RunSnapshot {
    /// Unique run identifier.
    pub id: Uuid,
    /// Identity of the actor who initiated the run.
    pub actor_id: Uuid,
    /// Timestamp when the run was created.
    #[schemars(with = "String")]
    pub created_at: Timestamp,
    /// Timestamp when execution actually started, if applicable.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<String>")]
    pub started_at: Option<Timestamp>,
    /// Per-node snapshots.
    pub nodes: Vec<NodeSnapshot>,
    /// Run outcome with state-specific data.
    pub outcome: RunOutcome,
}

/// Lightweight summary of a run for listing endpoints.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RunEntry {
    /// Unique run identifier.
    pub id: Uuid,
    /// Identity of the actor who initiated the run.
    pub actor_id: Uuid,
    /// Current overall status.
    pub status: RunStatus,
    /// Timestamp when the run was created.
    #[schemars(with = "String")]
    pub created_at: Timestamp,
    /// Timestamp when execution actually started, if applicable.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<String>")]
    pub started_at: Option<Timestamp>,
    /// Timestamp when the run finished, if applicable.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<String>")]
    pub completed_at: Option<Timestamp>,
    /// Number of phases executed in the run.
    pub node_count: usize,
    /// Total entities detected across all phases.
    pub entities_detected: u64,
    /// Total redactions applied across all nodes.
    pub redactions_applied: u64,
}

/// Filter criteria for listing runs.
#[derive(Debug, Clone, Default)]
pub struct RunFilter {
    /// If set, only return runs with this status.
    pub status: Option<RunStatus>,
}
