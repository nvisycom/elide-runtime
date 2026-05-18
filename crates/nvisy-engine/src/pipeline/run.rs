//! Single pipeline run lifecycle.
//!
//! [`Pipeline`] encapsulates the full lifecycle of one pipeline run:
//! validation, compilation, resource acquisition, execution,
//! retention enforcement, and finalization. It is created per-run by
//! [`Engine::run`] or [`Engine::submit`] and is not reusable.
//!
//! [`Engine::run`]: super::Engine::run
//! [`Engine::submit`]: super::Engine::submit

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use nvisy_core::Error;
use nvisy_ontology::policy::{Policies, Retention, RetentionPolicy, RetentionScope};
use nvisy_ontology::workflow::GraphNodeKind;
use nvisy_provider::http::HttpClient;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::config::RuntimeConfig;
use super::default::{EngineInput, EngineOutput};
use super::orchestrator::{Orchestrator, RunContext};
use super::plan::{self, ExecutionPlan};
use super::runs::RunStatus;
use super::runs::state::{RunRecord, RunState};
use crate::operation::envelope::SharedData;
use crate::registry::Registry;
use crate::utility::encryption::SharedKeyProvider;

const TARGET: &str = "nvisy_engine::pipeline::run";

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
        let compiled = self.prepare(&input).await?;
        self.execute(input, compiled).await
    }

    /// Validate input, compile the graph, and insert the initial run record.
    ///
    /// On success, returns the compiled plan. On compilation failure,
    /// inserts a failed run record and returns the error.
    pub async fn prepare(&self, input: &EngineInput) -> Result<ExecutionPlan, Error> {
        let compiled = match plan::compile(&input.graph) {
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
                            cancel: CancellationToken::new(),
                            entities_detected: 0,
                            redactions_applied: 0,
                            error: Some(e.to_string()),
                        },
                    )
                    .await;
                return Err(e);
            }
        };

        // Insert initial run record as Pending.
        self.runs
            .insert(
                self.run_id,
                RunRecord {
                    actor_id: input.actor_id,
                    status: RunStatus::Pending,
                    created_at: jiff::Timestamp::now(),
                    started_at: None,
                    completed_at: None,
                    nodes: HashMap::new(),
                    cancel: CancellationToken::new(),
                    entities_detected: 0,
                    redactions_applied: 0,
                    error: None,
                },
            )
            .await;

        Ok(compiled)
    }

    /// Run the compiled plan to completion.
    pub async fn execute(
        &self,
        input: EngineInput,
        compiled: ExecutionPlan,
    ) -> Result<EngineOutput, Error> {
        let effective_config = match &input.config {
            Some(overrides) => self.base_config.merge(overrides),
            None => self.base_config.clone(),
        };

        let actor_id = input.actor_id;

        // Acquire contexts and policies into the registry caches.
        let (_context_guard, _policy_guard) = self
            .acquire_resources(actor_id, &compiled.context_ids, &input.policy_ids)
            .await;

        let cached_policies = self
            .registry
            .policy_cache()
            .resolve(&input.policy_ids)
            .await;
        let mut policies = Policies::default();
        for policy in cached_policies {
            policies.push(Arc::unwrap_or_clone(policy));
        }

        let retention_rules = policies
            .all_retention()
            .into_iter()
            .copied()
            .collect::<Vec<_>>();

        let concurrency = input
            .graph
            .concurrency
            .or_else(|| effective_config.effective_concurrency());

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

        let cancel = CancellationToken::new();
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

        let limits = self.base_config.effective_limits();
        let orchestrator = Orchestrator::new(ctx);
        let run_output = if let Some(ms) = limits.run_timeout_ms {
            match tokio::time::timeout(
                Duration::from_millis(ms),
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

        // Save contexts from cache to registry (phase 6).
        if !input.dry_run {
            self.release_resources(actor_id, &compiled.save_context_ids)
                .await;
        }

        // Collect audits and counters from document results.
        let mut audits = Vec::new();
        let mut entities_detected = 0u64;
        let mut redactions_applied = 0u64;
        let mut any_failed = false;
        let mut any_ok = false;

        for result in &run_output.results {
            if result.error.is_some() {
                any_failed = true;
                continue;
            }
            any_ok = true;
            if let Some(ref envelope) = result.envelope {
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

        // Enforce retention policies.
        self.apply_retention(actor_id, &retention_rules, &input.graph, &mut output)
            .await;

        let status = if !any_failed {
            RunStatus::Succeeded
        } else if any_ok {
            RunStatus::PartialFailure
        } else {
            RunStatus::Failed
        };

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

    /// Acquire contexts and policies into the registry caches.
    async fn acquire_resources(
        &self,
        actor_id: Uuid,
        context_ids: &[Uuid],
        policy_ids: &[Uuid],
    ) -> (
        crate::registry::ResourceGuard<nvisy_ontology::context::Context>,
        crate::registry::ResourceGuard<nvisy_ontology::policy::Policy>,
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

    /// Persist contexts from the cache to the registry.
    ///
    /// Mirrors [`acquire_resources`] — contexts are loaded into the
    /// cache before execution and saved back here after completion.
    async fn release_resources(&self, actor_id: Uuid, save_context_ids: &[Uuid]) {
        let registry = &self.registry;
        for &id in save_context_ids {
            let context = match registry.context_cache().get(&id).await {
                Some(ctx) => ctx,
                None => {
                    tracing::warn!(
                        target: TARGET,
                        %id,
                        "context not found in cache, skipping save",
                    );
                    continue;
                }
            };
            if let Err(e) = registry
                .register_context(actor_id, Arc::unwrap_or_clone(context))
                .await
            {
                tracing::warn!(
                    target: TARGET,
                    %id,
                    error = %e,
                    "failed to save context",
                );
            }
        }
    }

    /// Enforce retention policies after a pipeline run.
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
