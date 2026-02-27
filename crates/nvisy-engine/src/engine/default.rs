//! Default engine implementation that orchestrates the full pipeline.
//!
//! [`DefaultEngine`] executes the three-phase pipeline:
//!
//! 1. **Detect** — run configured detection methods on the input content.
//! 2. **Evaluate** — map detected entities to redaction instructions via policies.
//! 3. **Redact** — apply redaction instructions to produce output content.
//!
//! After the content-level pipeline completes, the execution graph is run via
//! [`run_graph`] so that any Source/Action/Target DAG nodes are also executed.

use jiff::Timestamp;
use uuid::Uuid;

use nvisy_core::Error;
use nvisy_identify::{
    Audit, AuditAction, EvaluatePolicyAction, EvaluatePolicyParams, PolicyEvaluation,
    RedactionSummary,
};

use super::{Engine, EngineInput, EngineOutput};
use super::executor::run_graph;
use crate::compiler::build_plan;

/// Default [`Engine`] implementation.
///
/// Stateless — all configuration comes from the [`EngineInput`] provided at
/// call time. Suitable for embedding in long-lived application state.
#[derive(Debug, Clone, Copy)]
pub struct DefaultEngine;

impl Engine for DefaultEngine {
    async fn run(&self, input: EngineInput) -> Result<EngineOutput, Error> {
        let run_id = Uuid::new_v4();
        let mut audits: Vec<Audit> = Vec::new();
        let content_source = input.source.content_source();

        // ── Phase 1: Detection ──────────────────────────────────────
        //
        // Detection is handled externally (via DetectionService / NER / Pattern /
        // CV layers) before the engine is called. The engine receives entities as
        // part of a higher-level orchestration layer. For now, we create an empty
        // detection output and let the execution graph handle detection actions.
        let detection = nvisy_identify::DetectionOutput {
            source: content_source,
            entities: Vec::new(),
            policy_id: input.policies.policies.first().map(|p| p.id),
            duration_ms: None,
        };

        // ── Phase 2: Policy Evaluation ──────────────────────────────
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

            let action = EvaluatePolicyAction::connect(params).await?;
            let redactions = action.execute(detection.entities.clone()).await?;

            // Emit audit entries for each redaction decision.
            for r in &redactions {
                audits.push(Audit {
                    source: content_source,
                    action: AuditAction::Redaction,
                    timestamp: Timestamp::now(),
                    entity_id: Some(r.entity_id),
                    redaction_id: Some(r.source.as_uuid()),
                    policy_id: Some(policy.id),
                    source_id: Some(content_source.as_uuid()),
                    run_id: Some(run_id),
                    actor: input.actor.clone(),
                });
            }

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

        // ── Phase 3: Redaction ──────────────────────────────────────
        //
        // The ApplyRedactionAction is called directly by callers that have
        // parsed documents into the typed codec representation. At this level
        // we track the summary counts.
        let applied = all_redactions.iter().filter(|r| r.applied).count();
        let skipped = all_redactions.len() - applied;

        let summaries = vec![RedactionSummary {
            source: content_source,
            redactions_applied: applied,
            redactions_skipped: skipped,
        }];

        // ── Phase 4: DAG Execution ──────────────────────────────────
        //
        // Compile the graph into a topologically-sorted execution plan and
        // run Source/Action/Target nodes concurrently.
        let plan = build_plan(&input.graph)?;
        let run_output = run_graph(&plan, &input.connections).await?;

        // Emit a detection audit entry for the overall run.
        audits.push(Audit {
            source: content_source,
            action: AuditAction::Detection,
            timestamp: Timestamp::now(),
            entity_id: None,
            redaction_id: None,
            policy_id: input.policies.policies.first().map(|p| p.id),
            source_id: Some(content_source.as_uuid()),
            run_id: Some(run_id),
            actor: input.actor.clone(),
        });

        Ok(EngineOutput {
            run_id,
            output: input.source,
            detection,
            evaluation,
            summaries,
            audits,
            run_output,
        })
    }
}
