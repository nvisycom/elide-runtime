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
///
/// Engines and configs are held by value (not wrapped in `Arc`)
/// because below the top-level [`EngineInner`] singleton there's
/// no sharing — `RunContext` is built per-run, lives for that run,
/// then hands each phase its own copy via
/// [`DocumentPipeline::from_context`]. The engines themselves
/// internally hold `Arc`-wrapped recognizers / extractors, so
/// cloning them is a few atomic increments.
///
/// [`EngineInner`]: crate::pipeline::Engine
/// [`DocumentPipeline::from_context`]: crate::pipeline::DocumentPipeline::from_context
pub struct RunContext {
    /// Token to signal cancellation to all tasks.
    pub(crate) cancel: CancellationToken,
    /// Shared run-wide state: run ID, actor, registry, policies.
    pub(crate) shared: Arc<SharedData>,
    /// Pre-built extractor registry.
    pub(crate) extraction_engine: ExtractionEngine,
    /// Pre-built detection engine. Always present; when the plan
    /// requests no recognizers the engine is built empty and the
    /// per-modality dispatch short-circuits on empty recognizer
    /// lists.
    pub(crate) detection_engine: DetectionEngine,
    /// Server-wide redaction defaults. Per-plan `Redaction` fields
    /// fall back to these.
    pub(crate) redaction_config: RedactionConfig,
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
        extraction_engine: ExtractionEngine,
        detection_engine: DetectionEngine,
        redaction_config: RedactionConfig,
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
    pub(crate) fn extraction_engine(&self) -> &ExtractionEngine {
        &self.extraction_engine
    }

    /// Pre-built detection engine borrowed by [`DetectionPhase`].
    ///
    /// [`DetectionPhase`]: crate::detection::DetectionPhase
    pub(crate) fn detection_engine(&self) -> &DetectionEngine {
        &self.detection_engine
    }

    /// Server-wide redaction defaults the [`RedactionPhase`] reads.
    ///
    /// [`RedactionPhase`]: crate::redaction::RedactionPhase
    pub(crate) fn redaction_config(&self) -> &RedactionConfig {
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
