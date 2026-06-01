//! [`RunContext`]: per-run shared state every [`Phase`] reads from.
//!
//! Built once per run by the pipeline orchestrator from the
//! deployment-wide [`RuntimeConfig`] and the per-request
//! [`EngineInput`]; borrowed read-only by every [`PhaseContext`] in
//! the phase loop.
//!
//! Lives in `core/` (not `pipeline/`) because phases consume it
//! through `ctx.run.X` — it's part of the phase contract surface,
//! not the orchestrator's private state. The orchestrator constructs
//! it; phases read from it; nothing else mutates it.
//!
//! [`Phase`]: super::Phase
//! [`PhaseContext`]: super::PhaseContext
//! [`RuntimeConfig`]: crate::pipeline::RuntimeConfig
//! [`EngineInput`]: crate::pipeline::EngineInput

use std::num::NonZeroUsize;
use std::sync::Arc;

use tokio_util::sync::CancellationToken;

use super::SharedData;
use crate::detection::DetectionEngine;
use crate::extraction::ExtractionEngine;
use crate::redaction::RedactionConfig;

/// Per-run execution context shared across all document tasks.
pub struct RunContext {
    /// Token to signal cancellation to all tasks.
    pub(crate) cancel: CancellationToken,
    /// Shared run-wide state: run ID, actor, registry, policies.
    pub(crate) shared: Arc<SharedData>,
    /// Pre-built extractor registry from `RuntimeConfig.extraction`.
    /// Shared across every run.
    pub(crate) extraction_engine: Arc<ExtractionEngine>,
    /// Shared detection engine. Always present; when the plan
    /// requests no recognizers, the engine is built empty and the
    /// per-modality dispatch short-circuits on empty recognizer
    /// lists.
    pub(crate) detection_engine: Arc<DetectionEngine>,
    /// Server-wide redaction defaults from `RuntimeConfig.redaction`.
    /// Per-plan `Redaction` fields fall back to these.
    pub(crate) redaction_config: Arc<RedactionConfig>,
    /// Optional limit on how many documents may process concurrently.
    pub(crate) concurrency: Option<NonZeroUsize>,
    /// When `true`, skip redaction, validation, and export phases.
    pub(crate) dry_run: bool,
}

impl RunContext {
    /// Construct a [`RunContext`] from its parts. Called once per run
    /// by the orchestrator.
    pub(crate) fn new(
        cancel: CancellationToken,
        shared: Arc<SharedData>,
        extraction_engine: Arc<ExtractionEngine>,
        detection_engine: Arc<DetectionEngine>,
        redaction_config: Arc<RedactionConfig>,
        concurrency: Option<NonZeroUsize>,
        dry_run: bool,
    ) -> Self {
        Self {
            cancel,
            shared,
            extraction_engine,
            detection_engine,
            redaction_config,
            concurrency,
            dry_run,
        }
    }

    /// True when this run's cancellation token has fired.
    pub(crate) fn is_cancelled(&self) -> bool {
        self.cancel.is_cancelled()
    }

    /// Pre-built extraction engine borrowed by [`ExtractionPhase`].
    ///
    /// [`ExtractionPhase`]: crate::extraction::ExtractionPhase
    pub(crate) fn extraction_engine(&self) -> &Arc<ExtractionEngine> {
        &self.extraction_engine
    }

    /// Pre-built detection engine borrowed by [`DetectionPhase`].
    ///
    /// [`DetectionPhase`]: crate::detection::DetectionPhase
    pub(crate) fn detection_engine(&self) -> &Arc<DetectionEngine> {
        &self.detection_engine
    }

    /// Server-wide redaction defaults the [`RedactionPhase`] reads.
    ///
    /// [`RedactionPhase`]: crate::redaction::RedactionPhase
    pub(crate) fn redaction_config(&self) -> &Arc<RedactionConfig> {
        &self.redaction_config
    }

    /// Run-wide shared state (policies, registry, key provider).
    pub(crate) fn shared(&self) -> &Arc<SharedData> {
        &self.shared
    }

    /// Optional document-concurrency cap from `[engine.limits]`.
    pub(crate) fn concurrency(&self) -> Option<NonZeroUsize> {
        self.concurrency
    }

    /// True when this run skips redaction, validation, and export.
    pub(crate) fn dry_run(&self) -> bool {
        self.dry_run
    }
}
