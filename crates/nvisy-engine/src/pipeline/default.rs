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

use tokio::sync::{mpsc, watch};
use tokio::task::JoinSet;
use uuid::Uuid;

use nvisy_core::Error;
use nvisy_core::io::ContentData;
use nvisy_ontology::policy::{PolicyEvaluation, RedactionSummary};
use crate::operation::Operation;
use crate::operation::processing::{EvaluatePolicy, EvaluatePolicyParams};
use crate::provenance::{
    AuditEntryStatus, FileAudit, FileAuditEntryBuilder,
    ProcessingActionBuilder, ProcessingKind,
    FileAuditEntryKind,
};
use nvisy_ontology::record::RedactionMap;

use super::{Engine, EngineInput, EngineOutput};
use super::connections::Connections;
use super::executor::{NodeOutput, RunOutput, execute_node};
use crate::compiler::Compiler;
use crate::compiler::ExecutionPlan;

/// Default buffer size for bounded inter-node MPSC channels.
const CHANNEL_BUFFER_SIZE: usize = 256;

/// Default [`Engine`] implementation.
///
/// Stateless: all configuration comes from the [`EngineInput`] provided at
/// call time. Suitable for embedding in long-lived application state.
#[derive(Debug, Clone, Copy)]
pub struct DefaultEngine;

impl DefaultEngine {
    /// Execute a compiled [`ExecutionPlan`] by spawning concurrent tasks for
    /// each node.
    async fn run_graph(
        plan: &ExecutionPlan,
        connections: &Connections,
    ) -> Result<RunOutput, Error> {
        let run_id = Uuid::new_v4();
        let connections = Arc::new(connections.clone());

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
            let conns = Arc::clone(&connections);

            join_set.spawn(async move {
                // Wait for upstream nodes to complete
                for mut rx in upstream_watches {
                    let _ = rx.wait_for(|&done| done).await;
                }

                let result = execute_node(&node, node_senders, node_receivers, &conns).await;

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
        let content_source = input.source.content_source();

        // Contexts are accepted for future use by detection actions.
        let _contexts = &input.contexts;

        // Initialize per-file processing log and redaction map.
        let mut file_audit = FileAudit::new(content_source);
        file_audit.run_id = Some(run_id);
        let redaction_map = RedactionMap::new(content_source, run_id);

        // Phase 1: Detection
        //
        // Detection is handled externally (via DetectionService / NER / Pattern /
        // CV layers) before the engine is called. The engine receives entities as
        // part of a higher-level orchestration layer. For now, we create an empty
        // detection output and let the execution graph handle detection actions.
        let mut detection = nvisy_ontology::entity::DetectionOutput::new(content_source, Vec::new());
        detection.policy_id = input.policies.policies.first().map(|p| p.id);

        // Phase 2: Policy Evaluation
        //
        // Evaluate each policy against the detected entities to produce
        // redaction instructions, review holds, alerts, blocks, etc.
        let mut all_redactions = Vec::new();
        let mut evaluations = Vec::new();

        for policy in &input.policies.policies {
            let params = EvaluatePolicyParams {
                rules: policy.rules.clone(),
                default_spec: policy.default_spec.clone(),
                default_confidence_threshold: policy.default_confidence_threshold,
            };

            let action = EvaluatePolicy::connect(params).await?;
            let redactions = action.execute(detection.entities.clone()).await?;

            // Record a policy evaluation audit entry.
            let eval_entry = FileAuditEntryBuilder::default()
                .with_status(AuditEntryStatus::Success)
                .with_policy_id(policy.id)
                .with_kind(FileAuditEntryKind::Processing(
                    ProcessingKind::PolicyEvaluation(
                        ProcessingActionBuilder::default()
                            .with_matched_count(redactions.len() as u64)
                            .build()
                            .unwrap_or_default(),
                    ),
                ))
                .finish()
                .expect("valid audit entry");
            file_audit.push(eval_entry);

            evaluations.push(PolicyEvaluation {
                policy_id: policy.id,
                redactions: redactions.clone(),
                pending_review: Vec::new(),
                suppressed: Vec::new(),
                blocked: Vec::new(),
                alerted: Vec::new(),
            });

            all_redactions.extend(redactions);
        }

        // Use the first policy evaluation as the primary; merge if multiple.
        let evaluation = if let Some(first) = evaluations.into_iter().next() {
            first
        } else {
            PolicyEvaluation {
                policy_id: Uuid::nil(),
                redactions: Vec::new(),
                pending_review: Vec::new(),
                suppressed: Vec::new(),
                blocked: Vec::new(),
                alerted: Vec::new(),
            }
        };

        // Phase 3: Redaction
        //
        // The Redaction operation wraps per-modality logic (text, image, audio,
        // tabular). It requires typed `Document<T>` representations which are
        // not yet available at this level: the engine works with `ContentHandle`.
        // Once codec parsing is wired in, the call below will pass real documents
        // instead of empty vecs.
        let redaction_op = crate::operation::processing::Redaction;
        let redaction_input = crate::operation::processing::RedactionInput {
            text_docs: Vec::new(),
            image_docs: Vec::new(),
            audio_docs: Vec::new(),
            tabular_docs: Vec::new(),
            entities: detection.entities.clone(),
            redactions: all_redactions.clone(),
        };
        let _redaction_output = redaction_op.call(
            crate::operation::ParallelContext::new(redaction_input),
        ).await?;

        let applied = all_redactions.iter().filter(|r| r.applied).count();
        let skipped = all_redactions.len() - applied;

        // Record a redaction audit entry.
        let redaction_entry = FileAuditEntryBuilder::default()
            .with_status(AuditEntryStatus::Success)
            .with_kind(FileAuditEntryKind::Processing(
                ProcessingKind::Redaction(
                    ProcessingActionBuilder::default()
                        .with_items_count(all_redactions.len() as u64)
                        .with_matched_count(applied as u64)
                        .build()
                        .unwrap_or_default(),
                ),
            ))
            .finish()
            .expect("valid audit entry");
        file_audit.push(redaction_entry);

        let summaries = vec![RedactionSummary {
            source: content_source,
            redactions_applied: applied,
            redactions_skipped: skipped,
        }];

        // Phase 4: DAG Execution
        //
        // Compile the graph into a topologically-sorted execution plan and
        // run Source/Action/Target nodes concurrently.
        let mut compiler = Compiler::new();
        if let Some(retry) = input.default_retry {
            compiler = compiler.with_retry(retry);
        }
        if let Some(timeout) = input.default_timeout {
            compiler = compiler.with_timeout(timeout);
        }
        let plan = compiler.compile(&input.graph)?;
        let run_output = Self::run_graph(&plan, &input.connections).await?;

        Ok(EngineOutput {
            run_id,
            output: input.source,
            detection,
            evaluation,
            summaries,
            file_audits: vec![file_audit],
            redaction_maps: vec![redaction_map],
            run_output,
        })
    }
}
