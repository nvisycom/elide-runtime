//! Single pipeline run lifecycle.
//!
//! [`Pipeline`] encapsulates the full lifecycle of one pipeline run:
//! validation, compilation, resource acquisition, DAG execution,
//! retention enforcement, and finalization. It is created per-run by
//! [`Engine::run`] or [`Engine::submit`] and is not reusable.
//!
//! Separating the per-run lifecycle from [`Engine`] keeps the engine
//! as a thin facade (construction, CRUD, shutdown) while the pipeline
//! owns all execution state.
//!
//! [`Engine`]: super::Engine
//! [`Engine::run`]: super::Engine::run
//! [`Engine::submit`]: super::Engine::submit

use std::collections::HashMap;
use std::sync::Arc;

use nvisy_core::Error;
use nvisy_ontology::policy::{Policies, Retention, RetentionPolicy, RetentionScope};
use nvisy_ontology::workflow::{ConcurrencyPolicy, GraphNodeKind};
use nvisy_provider::http::HttpClient;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;
use validator::Validate;

use super::config::{ResourceLimits, RuntimeConfig};
use super::default::{EngineInput, EngineOutput};
use super::orchestrator::{Orchestrator, RunContext, StatusUpdate};
use super::plan;
use super::runs::state::{RunRecord, RunState};
use super::runs::{NodeSnapshot, NodeStatus, RunStatus};
use crate::operation::encryption::SharedKeyProvider;
use crate::operation::envelope::SharedData;
use crate::registry::{Registry, ResourceGuard};

const TARGET: &str = "nvisy_engine::pipeline::run";

/// Per-run state produced by [`Pipeline::prepare`] and consumed by
/// [`Pipeline::execute`].
pub(super) struct PreparedRun {
    cancel: CancellationToken,
    effective_config: RuntimeConfig,
    context_ids: Vec<Uuid>,
    limits: ResourceLimits,
    concurrency: Option<ConcurrencyPolicy>,
}

/// A single pipeline run lifecycle.
///
/// Created per-run, not reusable. Owns the run ID, cancellation
/// token, and all intermediate state needed to drive the run from
/// preparation through finalization.
pub(super) struct Pipeline {
    run_id: Uuid,
    registry: Registry,
    http_client: HttpClient,
    key_provider: Option<SharedKeyProvider>,
    runs: RunState,
    base_config: RuntimeConfig,
}

impl Pipeline {
    /// Create a new pipeline run bound to the given engine state.
    pub fn new(
        registry: Registry,
        http_client: HttpClient,
        key_provider: Option<SharedKeyProvider>,
        runs: RunState,
        base_config: RuntimeConfig,
    ) -> Self {
        Self {
            run_id: Uuid::now_v7(),
            registry,
            http_client,
            key_provider,
            runs,
            base_config,
        }
    }

    /// The unique identifier for this run.
    pub fn id(&self) -> Uuid {
        self.run_id
    }

    /// Prepare and execute the full pipeline, returning the output.
    ///
    /// This is the synchronous (blocking) path used by [`Engine::run`].
    ///
    /// [`Engine::run`]: super::Engine::run
    pub async fn run(self, input: EngineInput) -> Result<EngineOutput, Error> {
        let (compiled, prepared) = self.prepare(&input).await?;
        self.execute(input, compiled, prepared).await
    }

    /// Validate input, compile the graph, and insert the initial run record.
    ///
    /// On success, returns the compiled plan and prepared state needed
    /// by [`execute`]. On compilation failure, inserts a failed run
    /// record and returns the error.
    pub async fn prepare(
        &self,
        input: &EngineInput,
    ) -> Result<(plan::ExecutionPlan, PreparedRun), Error> {
        let cancel = CancellationToken::new();

        let effective_config = match &input.config {
            Some(overrides) => self.base_config.merge(overrides),
            None => self.base_config.clone(),
        };

        let mut context_ids: Vec<Uuid> = input
            .graph
            .nodes
            .iter()
            .filter_map(|node| match &node.kind {
                GraphNodeKind::LoadContext(cfg) => Some(cfg.context_ids.clone()),
                _ => None,
            })
            .flatten()
            .collect();
        context_ids.sort_unstable();
        context_ids.dedup();

        let limits = effective_config.effective_limits();
        let concurrency = input
            .graph
            .concurrency
            .or_else(|| effective_config.effective_concurrency());

        if let Some(ref c) = concurrency {
            c.validate()
                .map_err(|e| Error::validation(e.to_string(), "concurrency"))?;
        }

        let compiled = match plan::compile(&input.graph, limits.channel_buffer) {
            Ok(plan) => plan,
            Err(e) => {
                let now = jiff::Timestamp::now();
                self.runs
                    .insert(
                        self.run_id,
                        RunRecord {
                            actor_id: input.actor_id,
                            status: RunStatus::Failed,
                            created_at: now,
                            started_at: None,
                            completed_at: Some(now),
                            nodes: HashMap::new(),
                            cancel: cancel.clone(),
                            entities_detected: 0,
                            redactions_applied: 0,
                            error: Some(e.to_string()),
                        },
                    )
                    .await;
                return Err(e);
            }
        };

        let initial_nodes: HashMap<Uuid, NodeSnapshot> = compiled
            .nodes()
            .iter()
            .map(|n| {
                (
                    n.node.id,
                    NodeSnapshot {
                        node_id: n.node.id,
                        status: NodeStatus::Pending,
                        items_processed: 0,
                        error: None,
                    },
                )
            })
            .collect();

        self.runs
            .insert(
                self.run_id,
                RunRecord {
                    actor_id: input.actor_id,
                    status: RunStatus::Pending,
                    created_at: jiff::Timestamp::now(),
                    started_at: None,
                    completed_at: None,
                    nodes: initial_nodes,
                    cancel: cancel.clone(),
                    entities_detected: 0,
                    redactions_applied: 0,
                    error: None,
                },
            )
            .await;

        let prepared = PreparedRun {
            cancel,
            effective_config,
            context_ids,
            limits,
            concurrency,
        };

        Ok((compiled, prepared))
    }

    /// Run the compiled plan to completion.
    pub async fn execute(
        &self,
        input: EngineInput,
        compiled: plan::ExecutionPlan,
        prepared: PreparedRun,
    ) -> Result<EngineOutput, Error> {
        let PreparedRun {
            cancel,
            effective_config,
            context_ids,
            limits,
            concurrency,
        } = prepared;

        let actor_id = input.actor_id;
        let (_context_guard, _policy_guard) =
            self.acquire_resources(actor_id, &context_ids, &input.policy_ids)
                .await;

        let cached_policies = self
            .registry
            .policy_cache()
            .resolve(&input.policy_ids)
            .await;
        let mut policies = Policies::default();
        for policy in cached_policies {
            policies.push(std::sync::Arc::unwrap_or_clone(policy));
        }

        let retention_rules = policies
            .all_retention()
            .into_iter()
            .copied()
            .collect::<Vec<_>>();

        let mut shared_data = SharedData {
            run_id: self.run_id,
            actor_id,
            policies,
            registry: self.registry.clone(),
            key_provider: SharedKeyProvider::default(),
        };
        if let Some(ref kp) = self.key_provider {
            shared_data.key_provider = kp.clone();
        }

        let cancel_clone = cancel.clone();
        let ctx = RunContext {
            cancel,
            shared: Arc::new(shared_data),
            config: Arc::new(effective_config),
            http_client: self.http_client.clone(),
            concurrency,
            dry_run: input.dry_run,
        };

        self.runs.set_started_at(self.run_id).await;

        let on_status = self.status_callback();
        let orchestrator = Orchestrator::new(ctx, on_status);
        let run_output = if let Some(ms) = limits.run_timeout_ms {
            match tokio::time::timeout(
                std::time::Duration::from_millis(ms),
                orchestrator.run(&compiled),
            )
            .await
            {
                Ok(Ok(output)) => output,
                Ok(Err(e)) => {
                    self.runs.fail(self.run_id, e.to_string()).await;
                    return Err(e);
                }
                Err(_) => {
                    cancel_clone.cancel();
                    self.runs
                        .fail(self.run_id, "pipeline run exceeded time limit")
                        .await;
                    return Err(Error::timeout("pipeline run exceeded time limit"));
                }
            }
        } else {
            match orchestrator.run(&compiled).await {
                Ok(output) => output,
                Err(e) => {
                    self.runs.fail(self.run_id, e.to_string()).await;
                    return Err(e);
                }
            }
        };

        // Collect audits and counters from node results.
        let mut audits = Vec::new();
        let mut entities_detected = 0u64;
        let mut redactions_applied = 0u64;
        for nr in &run_output.node_results {
            for envelope in &nr.envelopes {
                entities_detected += envelope.audit.entities.len() as u64;
                redactions_applied += envelope
                    .audit
                    .entries
                    .iter()
                    .filter(|e| e.redaction.is_applied)
                    .count() as u64;
                audits.push(envelope.audit.clone());
            }
        }

        let mut output = EngineOutput {
            run_id: self.run_id,
            audits,
        };

        self.apply_retention(actor_id, &retention_rules, &input.graph, &mut output)
            .await;

        let status = run_output.run_status();
        self.runs
            .finalize(self.run_id, status, entities_detected, redactions_applied)
            .await;

        tracing::info!(
            target: TARGET,
            run_id = %self.run_id,
            ?status,
            entities_detected,
            redactions_applied,
            "pipeline run finalized",
        );

        Ok(output)
    }

    /// Build a status callback that bridges orchestrator node status
    /// updates to the in-memory [`RunState`].
    fn status_callback(&self) -> StatusUpdate {
        let runs = self.runs.clone();
        let run_id = self.run_id;
        Arc::new(move |node_id, status, items_processed, error| {
            let runs = runs.clone();
            Box::pin(async move {
                runs.update_node(run_id, node_id, status, items_processed, error)
                    .await;
            })
        })
    }

    /// Acquire contexts and policies into the registry caches.
    ///
    /// Returns RAII guards that release resources when dropped.
    async fn acquire_resources(
        &self,
        actor_id: Uuid,
        context_ids: &[Uuid],
        policy_ids: &[Uuid],
    ) -> (
        ResourceGuard<nvisy_ontology::context::Context>,
        ResourceGuard<nvisy_ontology::policy::Policy>,
    ) {
        let registry = &self.registry;

        let context_guard = registry
            .context_cache()
            .acquire(context_ids, |id| async move {
                match registry.read_context(actor_id, id).await {
                    Ok(ctx) => Some(ctx),
                    Err(e) => {
                        tracing::warn!(%id, error = %e, "failed to load context");
                        None
                    }
                }
            })
            .await;

        let policy_guard = registry
            .policy_cache()
            .acquire(policy_ids, |id| async move {
                match registry.read_policy(actor_id, id).await {
                    Ok(policy) => Some(policy),
                    Err(e) => {
                        tracing::warn!(%id, error = %e, "failed to load policy");
                        None
                    }
                }
            })
            .await;

        (context_guard, policy_guard)
    }

    /// Enforce retention policies after a pipeline run.
    ///
    /// For [`ZeroRetention`], applies immediately:
    /// - [`OriginalContent`]: deletes imported content from the registry
    /// - [`RedactedOutput`]: deletes exported content from the registry
    /// - [`AuditLogs`]: clears audits from the output and registry
    ///
    /// [`Duration`] and [`Indefinite`] retention are recorded as metadata
    /// but not enforced here (requires a background cleanup process).
    ///
    /// [`ZeroRetention`]: Retention::ZeroRetention
    /// [`OriginalContent`]: RetentionScope::OriginalContent
    /// [`RedactedOutput`]: RetentionScope::RedactedOutput
    /// [`AuditLogs`]: RetentionScope::AuditLogs
    /// [`Duration`]: Retention::Duration
    /// [`Indefinite`]: Retention::Indefinite
    async fn apply_retention(
        &self,
        actor_id: Uuid,
        retention_rules: &[RetentionPolicy],
        graph: &nvisy_ontology::workflow::Graph,
        output: &mut EngineOutput,
    ) {
        if retention_rules.is_empty() {
            return;
        }

        for rp in retention_rules {
            if !matches!(rp.retention, Retention::ZeroRetention) {
                continue;
            }
            match rp.scope {
                RetentionScope::OriginalContent => {
                    let content_ids: Vec<Uuid> = graph
                        .nodes
                        .iter()
                        .filter_map(|node| match &node.kind {
                            GraphNodeKind::ImportFile(cfg) => Some(cfg.content_ids.clone()),
                            _ => None,
                        })
                        .flatten()
                        .collect();
                    for id in content_ids {
                        if let Err(e) = self.registry.unregister_content(actor_id, id).await {
                            tracing::warn!(
                                %id,
                                error = %e,
                                "failed to delete content for zero-retention",
                            );
                        }
                    }
                }
                RetentionScope::RedactedOutput => {
                    let content_ids: Vec<Uuid> = graph
                        .nodes
                        .iter()
                        .filter_map(|node| match &node.kind {
                            GraphNodeKind::ExportFile(cfg) => Some(cfg.content_ids.clone()),
                            _ => None,
                        })
                        .flatten()
                        .collect();
                    for id in content_ids {
                        if let Err(e) = self.registry.unregister_content(actor_id, id).await {
                            tracing::warn!(
                                %id,
                                error = %e,
                                "failed to delete exported content for zero-retention",
                            );
                        }
                    }
                }
                RetentionScope::AuditLogs => {
                    output.audits.clear();
                    if let Err(e) = self
                        .registry
                        .unregister_audits(actor_id, output.run_id)
                        .await
                    {
                        tracing::warn!(
                            run_id = %output.run_id,
                            error = %e,
                            "failed to delete persisted audits for zero-retention",
                        );
                    }
                }
                _ => {}
            }
        }
    }
}
