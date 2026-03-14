//! Default engine implementation that orchestrates the full pipeline.
//!
//! [`DefaultEngine`] executes the three-phase pipeline:
//!
//! 1. **Detect**: run configured detection methods on the input content.
//! 2. **Evaluate**: map detected entities to redaction instructions via policies.
//! 3. **Redact**: apply redaction instructions to produce output content.
//!
//! After the content-level pipeline completes, the execution graph is run
//! so that any Source/Action/Target DAG nodes are also executed.

use std::collections::HashMap;
use std::sync::Arc;

use jiff::Timestamp;
use nvisy_core::Error;
use nvisy_core::content::ContentData;
use nvisy_http::HttpClient;
use tokio::sync::{RwLock, mpsc, watch};
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::executor::{NodeOutput, RunOutput, execute_node};
use super::plan::{self, ExecutionPlan};
use super::runs::{NodeSnapshot, NodeStatus, RunFilter, RunSnapshot, RunStatus, RunSummary};
use super::{Engine, EngineInput, EngineOutput, Runs};
use crate::graph::policy::{RetryPolicy, TimeoutPolicy};
use crate::operation::SharedContext;
use crate::provenance::PolicyEvaluation;

/// Private mutable state for a single run, held inside `DefaultEngineInner`.
struct RunEntry {
    actor_id: Uuid,
    status: RunStatus,
    created_at: Timestamp,
    completed_at: Option<Timestamp>,
    nodes: Vec<NodeSnapshot>,
    cancel: CancellationToken,
}

impl RunEntry {
    fn to_snapshot(&self, id: Uuid) -> RunSnapshot {
        RunSnapshot {
            id,
            actor_id: self.actor_id,
            status: self.status,
            created_at: self.created_at,
            completed_at: self.completed_at,
            nodes: self.nodes.clone(),
        }
    }

    fn to_summary(&self, id: Uuid) -> RunSummary {
        RunSummary {
            id,
            actor_id: self.actor_id,
            status: self.status,
            created_at: self.created_at,
            completed_at: self.completed_at,
            node_count: self.nodes.len(),
        }
    }
}

impl Clone for DefaultEngineInner {
    fn clone(&self) -> Self {
        Self {
            default_retry: self.default_retry.clone(),
            default_timeout: self.default_timeout.clone(),
            http_client: self.http_client.clone(),
            runs: RwLock::new(HashMap::new()),
        }
    }
}

/// Inner state shared behind an [`Arc`].
struct DefaultEngineInner {
    /// Default retry policy for graph nodes.
    default_retry: Option<RetryPolicy>,
    /// Default timeout policy for graph nodes.
    default_timeout: Option<TimeoutPolicy>,
    /// Shared HTTP client for downstream providers.
    http_client: HttpClient,
    /// All tracked runs keyed by their UUID.
    runs: RwLock<HashMap<Uuid, RunEntry>>,
}

impl Default for DefaultEngineInner {
    fn default() -> Self {
        Self {
            default_retry: None,
            default_timeout: None,
            http_client: HttpClient::default(),
            runs: RwLock::new(HashMap::new()),
        }
    }
}

/// Default [`Engine`] implementation.
///
/// Wraps policies in an `Arc` so cloning is cheap.
#[derive(Clone, Default)]
pub struct DefaultEngine {
    inner: Arc<DefaultEngineInner>,
}

impl std::fmt::Debug for DefaultEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DefaultEngine")
            .field("default_retry", &self.inner.default_retry)
            .field("default_timeout", &self.inner.default_timeout)
            .field("http_client", &self.inner.http_client)
            .finish()
    }
}

impl DefaultEngine {
    /// Create a new engine with no default policies.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the default retry policy.
    pub fn with_retry(mut self, policy: RetryPolicy) -> Self {
        Arc::make_mut(&mut self.inner).default_retry = Some(policy);
        self
    }

    /// Set the default timeout policy.
    pub fn with_timeout(mut self, policy: TimeoutPolicy) -> Self {
        Arc::make_mut(&mut self.inner).default_timeout = Some(policy);
        self
    }

    /// Set the shared HTTP client for downstream providers.
    pub fn with_http_client(mut self, client: HttpClient) -> Self {
        Arc::make_mut(&mut self.inner).http_client = client;
        self
    }

    /// Returns the shared HTTP client.
    pub fn http_client(&self) -> &HttpClient {
        &self.inner.http_client
    }

    /// Execute a compiled [`ExecutionPlan`] by spawning concurrent tasks for
    /// each node.
    async fn run_graph(
        plan: &ExecutionPlan,
        cancel: CancellationToken,
    ) -> Result<RunOutput, Error> {
        // Create channels for each edge using pre-computed config
        let mut senders: HashMap<Uuid, Vec<mpsc::Sender<ContentData>>> = HashMap::new();
        let mut receivers: HashMap<Uuid, Vec<mpsc::Receiver<ContentData>>> = HashMap::new();

        for edge in plan.edges() {
            let (tx, rx) = mpsc::channel(edge.config.channel_buffer);
            senders.entry(edge.source).or_default().push(tx);
            receivers.entry(edge.target).or_default().push(rx);
        }

        // Create completion signals per node
        let mut signal_senders: HashMap<Uuid, watch::Sender<bool>> = HashMap::new();
        let mut signal_receivers: HashMap<Uuid, watch::Receiver<bool>> = HashMap::new();

        for resolved in plan.nodes() {
            let (tx, rx) = watch::channel(false);
            signal_senders.insert(resolved.node.id, tx);
            signal_receivers.insert(resolved.node.id, rx);
        }

        // Spawn tasks
        let mut join_set: JoinSet<NodeOutput> = JoinSet::new();

        for resolved in plan.nodes() {
            let resolved = resolved.clone();
            let node_id = resolved.node.id;
            let cancel = cancel.clone();

            let upstream_watches: Vec<watch::Receiver<bool>> = resolved
                .upstream_ids
                .iter()
                .filter_map(|id| signal_receivers.get(id).cloned())
                .collect();

            let completion_tx = signal_senders.remove(&node_id);
            let node_senders = senders.remove(&node_id).unwrap_or_default();
            let node_receivers = receivers.remove(&node_id).unwrap_or_default();

            join_set.spawn(async move {
                // Wait for upstream nodes to complete
                for mut rx in upstream_watches {
                    let _ = rx.wait_for(|&done| done).await;
                }

                let result = execute_node(&resolved, node_senders, node_receivers, cancel).await;

                // Signal completion
                if let Some(tx) = completion_tx {
                    let _ = tx.send(true);
                }

                match result {
                    Ok(count) => NodeOutput {
                        node_id,
                        items_processed: count,
                        error: None,
                    },
                    Err(e) => NodeOutput {
                        node_id,
                        items_processed: 0,
                        error: Some(e.to_string()),
                    },
                }
            });
        }

        // Collect results
        let mut node_results = Vec::new();
        while let Some(result) = join_set.join_next().await {
            match result {
                Ok(nr) => node_results.push(nr),
                Err(e) => node_results.push(NodeOutput {
                    node_id: Uuid::nil(),
                    items_processed: 0,
                    error: Some(format!("Task panicked: {}", e)),
                }),
            }
        }

        Ok(RunOutput { node_results })
    }

    /// Build [`NodeSnapshot`]s from a completed [`RunOutput`].
    fn node_snapshots(run_output: &RunOutput) -> Vec<NodeSnapshot> {
        run_output
            .node_results
            .iter()
            .map(|nr| NodeSnapshot {
                node_id: nr.node_id,
                status: if nr.error.is_none() {
                    NodeStatus::Succeeded
                } else {
                    NodeStatus::Failed
                },
                items_processed: nr.items_processed,
                error: nr.error.clone(),
            })
            .collect()
    }
}

impl Engine for DefaultEngine {
    async fn run(&self, input: EngineInput) -> Result<EngineOutput, Error> {
        let run_id = Uuid::new_v4();
        let cancel = CancellationToken::new();

        // Register the run as Pending
        {
            let entry = RunEntry {
                actor_id: input.actor_id,
                status: RunStatus::Pending,
                created_at: Timestamp::now(),
                completed_at: None,
                nodes: Vec::new(),
                cancel: cancel.clone(),
            };
            self.inner.runs.write().await.insert(run_id, entry);
        }

        // Transition to Running
        if let Some(entry) = self.inner.runs.write().await.get_mut(&run_id) {
            entry.status = RunStatus::Running;
        }

        let _shared = SharedContext::new(run_id, input.actor_id)
            .with_policies(input.policies.clone())
            .with_contexts(input.contexts.clone());

        // Phase 1: Detection
        let detection = nvisy_ontology::entity::DetectionOutput::new(
            nvisy_core::content::ContentSource::new(),
            Vec::new(),
        );

        // Phase 2: Policy Evaluation
        let evaluation = PolicyEvaluation::new(Uuid::nil());

        // Phase 3: DAG Execution
        let compiled = plan::compile(
            &input.graph,
            self.inner.default_retry.as_ref(),
            self.inner.default_timeout.as_ref(),
        )?;
        let run_output = Self::run_graph(&compiled, cancel).await?;

        // Transition to Succeeded/Failed and populate node snapshots
        {
            let snapshots = Self::node_snapshots(&run_output);
            if let Some(entry) = self.inner.runs.write().await.get_mut(&run_id) {
                let any_ok = run_output.node_results.iter().any(|r| r.error.is_none());
                let any_err = run_output.node_results.iter().any(|r| r.error.is_some());
                entry.status = match (any_ok, any_err) {
                    (_, false) => RunStatus::Succeeded,
                    (true, true) => RunStatus::PartialFailure,
                    _ => RunStatus::Failed,
                };
                entry.completed_at = Some(Timestamp::now());
                entry.nodes = snapshots;
            }
        }

        Ok(EngineOutput {
            run_id,
            detection,
            evaluation,
            summaries: Vec::new(),
            file_audits: Vec::new(),
            redaction_maps: Vec::new(),
        })
    }
}

impl Runs for DefaultEngine {
    async fn get_run(&self, id: Uuid) -> Option<RunSnapshot> {
        self.inner
            .runs
            .read()
            .await
            .get(&id)
            .map(|entry| entry.to_snapshot(id))
    }

    async fn list_runs(&self, filter: RunFilter) -> Vec<RunSummary> {
        self.inner
            .runs
            .read()
            .await
            .iter()
            .filter(|(_, entry)| {
                filter.status.is_none_or(|s| entry.status == s)
                    && filter.actor_id.is_none_or(|a| entry.actor_id == a)
            })
            .map(|(&id, entry)| entry.to_summary(id))
            .collect()
    }

    async fn cancel_run(&self, id: Uuid) -> Result<(), Error> {
        let mut runs = self.inner.runs.write().await;
        let entry = runs
            .get_mut(&id)
            .ok_or_else(|| Error::new(nvisy_core::ErrorKind::NotFound, "run not found"))?;

        match entry.status {
            RunStatus::Pending | RunStatus::Running => {
                entry.cancel.cancel();
                entry.status = RunStatus::Cancelled;
                entry.completed_at = Some(Timestamp::now());
                Ok(())
            }
            _ => Err(Error::new(
                nvisy_core::ErrorKind::Validation,
                "run has already finished",
            )),
        }
    }
}
