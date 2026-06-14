//! Per-pass detection pipeline lifecycle.
//!
//! [`DetectionPipeline`] is created per [`DetectionEngine::detect`] call.
//! Owns the pass id, registry handle, run context, and
//! [`DetectionState`] accessors.
//!
//! [`DetectionEngine::detect`]: crate::detection::DetectionEngine::detect

use std::sync::Arc;

use jiff::Timestamp;
use nvisy_codec::CodecRegistry;
use nvisy_core::Error;
use nvisy_toolkit::extraction::ExtractorRegistry;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::input::DetectionInput;
use super::orchestrator::DetectionOrchestrator;
use super::result::DetectionResult;
use super::state::{DetectionRecord, DetectionState};
use super::status::DetectionStatus;
use crate::core::{DetectionContext, DetectionEngines, PolicyStore, SharedData};
use crate::document::provenance::AnyAudit;
use crate::core::ingestion::ImportFile;
use crate::core::ingestion::encryption::SharedKeyProvider;
use crate::core::RuntimeConfig;
use crate::detection::DetectionConfig;
use crate::policy::{Policy, PolicyDigest};
use crate::registry::Registry;

const TARGET: &str = "nvisy_engine::pipeline::detection::pipeline";

/// Pre-built engine resources the detection pipeline borrows for
/// the duration of one pass.
pub(crate) struct DetectionEngineState {
    pub extraction_engine: Arc<ExtractorRegistry>,
    pub detection_config: Arc<DetectionConfig>,
}

/// A single detection-pass lifecycle.
pub(crate) struct DetectionPipeline {
    detection_id: Uuid,
    registry: Registry,
    key_provider: Option<SharedKeyProvider>,
    detections: DetectionState,
    base_config: RuntimeConfig,
    state: DetectionEngineState,
}

impl DetectionPipeline {
    pub(crate) fn new(
        registry: Registry,
        key_provider: Option<SharedKeyProvider>,
        detections: DetectionState,
        base_config: RuntimeConfig,
        state: DetectionEngineState,
    ) -> Self {
        Self {
            detection_id: Uuid::now_v7(),
            registry,
            key_provider,
            detections,
            base_config,
            state,
        }
    }

    pub(crate) fn id(&self) -> Uuid {
        self.detection_id
    }

    /// Validate the submitted request, build the per-pass
    /// [`PolicyStore`] + digests, and register a pending detection
    /// record.
    ///
    /// Validation chains the three [`DetectionInput`] checks:
    /// policy/rule name uniqueness, label union with conflict
    /// detection, and selector-label resolution against the
    /// unioned catalog.
    ///
    /// # Errors
    ///
    /// Returns the first validation error raised by any of the
    /// three checks.
    pub(crate) async fn register_pending(
        &self,
        input: DetectionInput,
    ) -> Result<PreparedDetection, Error> {
        input.validate_namespace()?;
        let catalog = input.unify_labels()?;
        input.validate_selector_labels(&catalog)?;
        input.validate_actions()?;

        let actor_id = input.actor_id;
        let policy_digests: Vec<PolicyDigest> =
            input.policies.iter().map(Policy::digest).collect();
        let policies = Arc::new(PolicyStore::from_policies(input.policies));

        self.detections
            .insert(
                self.detection_id,
                DetectionRecord {
                    actor_id,
                    policies: Arc::clone(&policies),
                    policy_digests: policy_digests.clone(),
                    imports: input.imports.clone(),
                    status: DetectionStatus::Pending,
                    created_at: Timestamp::now(),
                    started_at: None,
                    completed_at: None,
                    cancel: CancellationToken::new(),
                    audits: Vec::new(),
                    entities_detected: 0,
                    error: None,
                },
            )
            .await;

        Ok(PreparedDetection {
            actor_id,
            policies,
            policy_digests,
            catalog,
            imports: input.imports,
            plan: input.plan,
        })
    }

    /// Run the detection pass to completion.
    pub(crate) async fn execute(
        &self,
        prepared: PreparedDetection,
    ) -> Result<(Vec<AnyAudit>, u64, DetectionStatus), Error> {
        let actor_id = prepared.actor_id;

        let (recognizer_registry, context_enhancer) = match self
            .state
            .detection_config
            .build_for_request(&prepared.catalog)
        {
            Ok(r) => (Arc::new(r.recognizers), Arc::new(r.enhancer)),
            Err(e) => {
                self.detections.fail(self.detection_id, e.to_string()).await;
                return Err(e);
            }
        };

        let mut shared_data = SharedData {
            run_id: self.detection_id,
            actor_id,
            policies: prepared.policies,
            catalog: Arc::new(prepared.catalog),
            registry: self.registry.clone(),
            codec_registry: CodecRegistry::with_builtin(),
            key_provider: SharedKeyProvider::default(),
        };
        if let Some(ref kp) = self.key_provider {
            shared_data.key_provider = kp.clone();
        }

        let cancel = CancellationToken::new();
        let engines = DetectionEngines {
            extraction_engine: (*self.state.extraction_engine).clone(),
            recognizer_registry,
            context_enhancer,
        };
        let concurrency = self.base_config.effective_concurrency();
        let ctx = DetectionContext::new(cancel, Arc::new(shared_data), engines, concurrency);

        self.detections.set_started_at(self.detection_id).await;

        let orchestrator = DetectionOrchestrator::new(ctx);
        let output = match orchestrator.run(&prepared.imports, &prepared.plan).await {
            Ok(out) => out,
            Err(e) => {
                self.detections.fail(self.detection_id, e.to_string()).await;
                return Err(e);
            }
        };

        let (audits, entities_detected, any_ok, any_failed) = output.into_audits();
        let status = if !any_failed {
            DetectionStatus::Succeeded
        } else if any_ok {
            DetectionStatus::PartialFailure
        } else {
            DetectionStatus::Failed
        };

        if matches!(
            status,
            DetectionStatus::Succeeded | DetectionStatus::PartialFailure
        ) {
            let result = DetectionResult {
                id: self.detection_id,
                actor_id,
                policies: prepared.policy_digests,
                imports: prepared.imports.clone(),
                audits: audits.clone(),
                entities_detected,
            };
            if let Err(e) = self
                .registry
                .store_detection(actor_id, self.detection_id, &result)
                .await
            {
                tracing::error!(
                    target: TARGET,
                    detection_id = %self.detection_id,
                    error = %e,
                    "failed to persist detection result; in-memory state stays valid",
                );
            }
        }

        self.detections
            .finalize(self.detection_id, status, audits.clone(), entities_detected)
            .await;

        tracing::info!(
            target: TARGET,
            detection_id = %self.detection_id,
            ?status,
            entities_detected,
            "detection pass finalized",
        );

        Ok((audits, entities_detected, status))
    }
}

/// Output of [`DetectionPipeline::register_pending`] — the
/// per-pass state that has to flow to [`DetectionPipeline::execute`]
/// without re-touching the original `Vec<Policy>`.
pub(crate) struct PreparedDetection {
    actor_id: Uuid,
    policies: Arc<PolicyStore>,
    policy_digests: Vec<PolicyDigest>,
    catalog: nvisy_core::entity::EntityLabelCatalog,
    imports: Vec<ImportFile>,
    plan: crate::detection::DetectionPlan,
}
