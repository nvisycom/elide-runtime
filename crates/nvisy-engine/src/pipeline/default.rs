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

use nvisy_core::Error;
use nvisy_core::content::ContentData;
use nvisy_http::HttpClient;
use tokio::sync::{mpsc, watch};
use tokio::task::JoinSet;
use uuid::Uuid;

use super::executor::{NodeOutput, RunOutput, execute_node};
use super::{Engine, EngineInput, EngineOutput};
use crate::compiler::{Compiler, ExecutionPlan, RetryPolicy, TimeoutPolicy};
use crate::operation::SharedContext;
use crate::provenance::PolicyEvaluation;

/// Default buffer size for bounded inter-node MPSC channels.
const CHANNEL_BUFFER_SIZE: usize = 256;

/// Inner state shared behind an [`Arc`].
#[derive(Clone, Default)]
struct DefaultEngineInner {
    /// Default retry policy for graph nodes without one.
    retry: Option<RetryPolicy>,
    /// Default timeout policy for graph nodes without one.
    timeout: Option<TimeoutPolicy>,
    /// Shared HTTP client for downstream providers.
    http_client: HttpClient,
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
            .field("retry", &self.inner.retry)
            .field("timeout", &self.inner.timeout)
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
        Arc::make_mut(&mut self.inner).retry = Some(policy);
        self
    }

    /// Set the default timeout policy.
    pub fn with_timeout(mut self, policy: TimeoutPolicy) -> Self {
        Arc::make_mut(&mut self.inner).timeout = Some(policy);
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
    async fn run_graph(plan: &ExecutionPlan) -> Result<RunOutput, Error> {
        let run_id = Uuid::new_v4();

        // Create channels for each edge
        let mut senders: HashMap<Uuid, Vec<mpsc::Sender<ContentData>>> = HashMap::new();
        let mut receivers: HashMap<Uuid, Vec<mpsc::Receiver<ContentData>>> = HashMap::new();

        for node in &plan.nodes {
            let node_id = node.node.id;
            for downstream_id in &node.downstream_ids {
                let (tx, rx) = mpsc::channel(CHANNEL_BUFFER_SIZE);
                senders.entry(node_id).or_default().push(tx);
                receivers.entry(*downstream_id).or_default().push(rx);
            }
        }

        // Create completion signals per node
        let mut signal_senders: HashMap<Uuid, watch::Sender<bool>> = HashMap::new();
        let mut signal_receivers: HashMap<Uuid, watch::Receiver<bool>> = HashMap::new();

        for node in &plan.nodes {
            let (tx, rx) = watch::channel(false);
            signal_senders.insert(node.node.id, tx);
            signal_receivers.insert(node.node.id, rx);
        }

        // Spawn tasks
        let mut join_set: JoinSet<NodeOutput> = JoinSet::new();

        for resolved in &plan.nodes {
            let node = resolved.node.clone();
            let node_id = node.id;
            let upstream_ids = resolved.upstream_ids.clone();

            let upstream_watches: Vec<watch::Receiver<bool>> = upstream_ids
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

                let result = execute_node(&node, node_senders, node_receivers).await;

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

        let success = node_results.iter().all(|r| r.error.is_none());

        Ok(RunOutput {
            run_id,
            node_results,
            success,
        })
    }
}

impl Engine for DefaultEngine {
    async fn run(&self, input: EngineInput) -> Result<EngineOutput, Error> {
        let run_id = Uuid::new_v4();

        let _shared = SharedContext::new(run_id, input.actor_id)
            .with_policies(input.policies.clone())
            .with_contexts(input.contexts.clone());

        // Phase 1: Detection
        //
        // Detection is handled externally (via DetectionService / NER / Pattern /
        // CV layers) before the engine is called. The engine receives entities as
        // part of a higher-level orchestration layer. For now, we create an empty
        // detection output and let the execution graph handle detection actions.
        let detection = nvisy_ontology::entity::DetectionOutput::new(
            nvisy_core::content::ContentSource::new(),
            Vec::new(),
        );

        // Phase 2: Policy Evaluation
        //
        // Policy evaluation is handled by the execution graph. For now we
        // produce an empty evaluation.
        let evaluation = PolicyEvaluation::new(Uuid::nil());

        // Phase 3: DAG Execution
        //
        // Compile the graph into a topologically-sorted execution plan and
        // run Source/Action/Target nodes concurrently.
        let mut compiler = Compiler::new();
        if let Some(ref retry) = self.inner.retry {
            compiler = compiler.with_retry(retry.clone());
        }
        if let Some(ref timeout) = self.inner.timeout {
            compiler = compiler.with_timeout(timeout.clone());
        }
        let plan = compiler.compile(&input.graph)?;
        let run_output = Self::run_graph(&plan).await?;

        Ok(EngineOutput {
            run_id,
            detection,
            evaluation,
            summaries: Vec::new(),
            file_audits: Vec::new(),
            redaction_maps: Vec::new(),
            run_output,
        })
    }
}
