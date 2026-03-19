//! Refinement handlers: fusion, redaction, and validation.

use nvisy_codec::handler::TextHandler;
use nvisy_core::Error;

use super::super::handler::NodeHandler;
use super::retry;
use crate::graph;
use crate::operation::processing::{
    Deduplication, Ensemble, EvaluatePolicy, EvaluatePolicyParams, FusionStrategy,
    Validation, ValidationInput,
};
use crate::operation::{DocumentEnvelope, Operation, ParallelContext, SharedContext};
use crate::pipeline::policy::CompiledRetryPolicy;

const TARGET: &str = "nvisy_engine::pipeline::executor";

pub(crate) struct FusionHandler {
    dedup: bool,
    shared: SharedContext,
    retry: Option<CompiledRetryPolicy>,
}

impl FusionHandler {
    pub fn new(cfg: &graph::Fusion, shared: SharedContext, retry: Option<CompiledRetryPolicy>) -> Self {
        if cfg.confidence_calibration {
            tracing::warn!(target: TARGET, "confidence_calibration not yet implemented, skipping");
        }
        if cfg.contextual_adjustment {
            tracing::warn!(target: TARGET, "contextual_adjustment not yet implemented, skipping");
        }
        Self {
            dedup: cfg.entity_deduplication,
            shared,
            retry,
        }
    }
}

#[async_trait::async_trait]
impl NodeHandler for FusionHandler {
    async fn handle(&self, mut envelope: DocumentEnvelope) -> Result<DocumentEnvelope, Error> {
        let retry = self.retry.as_ref();

        if self.dedup && !envelope.entities.is_empty() {
            let do_dedup = || {
                let entities = envelope.entities.clone();
                let shared = self.shared.clone();
                async move {
                    let input = ParallelContext::new(entities, shared);
                    Deduplication.call(input).await
                }
            };
            let output = retry::call(retry, do_dedup).await?;
            envelope.apply(output.into_inner());
        }

        if !envelope.entities.is_empty() {
            let do_ensemble = || {
                let entities = envelope.entities.clone();
                let shared = self.shared.clone();
                async move {
                    let input = ParallelContext::new(entities, shared);
                    Ensemble::new(FusionStrategy::MaxConfidence).call(input).await
                }
            };
            let output = retry::call(retry, do_ensemble).await?;
            envelope.apply(output.into_inner());
        }

        Ok(envelope)
    }
}

pub(crate) struct RedactionHandler {
    eval: EvaluatePolicy,
    shared: SharedContext,
    retry: Option<CompiledRetryPolicy>,
}

impl RedactionHandler {
    pub async fn new(
        cfg: &graph::Redaction,
        shared: SharedContext,
        retry: Option<CompiledRetryPolicy>,
    ) -> Result<Self, Error> {
        let rules = shared
            .policies
            .policies
            .iter()
            .flat_map(|p| p.rules.clone())
            .collect();

        let eval = EvaluatePolicy::connect(EvaluatePolicyParams {
            rules,
            default_spec: nvisy_ontology::policy::Strategy::Text(
                nvisy_ontology::policy::TextStrategy::Mask { mask_char: '*' },
            ),
            default_confidence_threshold: 0.5,
        })
        .await?;

        if cfg.process_metadata {
            tracing::debug!(target: TARGET, "metadata processing handled by policy evaluation");
        }

        Ok(Self { eval, shared, retry })
    }
}

#[async_trait::async_trait]
impl NodeHandler for RedactionHandler {
    async fn handle(&self, mut envelope: DocumentEnvelope) -> Result<DocumentEnvelope, Error> {
        if !envelope.entities.is_empty() {
            let eval_ref = &self.eval;
            let do_eval = || {
                let entities = envelope.entities.clone();
                let shared = self.shared.clone();
                async move {
                    let input = ParallelContext::new(entities, shared);
                    eval_ref.call(input).await
                }
            };
            let output = retry::call(self.retry.as_ref(), do_eval).await?;
            envelope.apply(output.into_inner());
        }
        Ok(envelope)
    }
}

pub(crate) struct ValidationHandler {
    fail_on_leak: bool,
    shared: SharedContext,
}

impl ValidationHandler {
    pub fn new(cfg: &graph::Validation, shared: SharedContext) -> Self {
        Self {
            fail_on_leak: cfg.fail_on_leak,
            shared,
        }
    }
}

#[async_trait::async_trait]
impl NodeHandler for ValidationHandler {
    async fn handle(&self, envelope: DocumentEnvelope) -> Result<DocumentEnvelope, Error> {
        use futures::StreamExt;
        use nvisy_codec::Document;

        let redacted_text = match &envelope.document {
            Document::Text(h) => {
                let spans: Vec<_> = h.text_spans().await.collect().await;
                let text: String = spans.iter().map(|s| s.data.as_str()).collect();
                Some(text)
            }
            _ => None,
        };

        let input = ValidationInput {
            entities: envelope.entities.clone(),
            decisions: envelope.audit.decisions.clone(),
            redacted_text,
        };

        let ctx = ParallelContext::new(input, self.shared.clone());
        let output = Validation.call(ctx).await?;
        let result = output.data;

        if !result.leaked.is_empty() {
            tracing::warn!(
                target: TARGET,
                leaked = result.leaked.len(),
                passed = result.passed,
                "validation found leaked values",
            );
            if self.fail_on_leak {
                return Err(nvisy_core::Error::validation(
                    format!("{} redacted values leaked in output", result.leaked.len()),
                    "validation",
                ));
            }
        } else {
            tracing::debug!(
                target: TARGET,
                passed = result.passed,
                "validation passed",
            );
        }

        Ok(envelope)
    }
}
