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

use nvisy_toolkit::detection::RecognizerRegistry;
use nvisy_toolkit::extraction::ExtractorRegistry;
use tokio_util::sync::CancellationToken;

use super::SharedData;
use crate::phases::redaction::RedactionRegistries;
use crate::pipeline::RedactionConfig;

/// Per-run execution context shared across all document tasks.
///
/// Engines and configs are held by value (not wrapped in `Arc`)
/// because below the top-level [`EngineInner`] singleton there's
/// no sharing — `RunContext` is built per-run, lives for that run,
/// then hands each phase its own copy via the pipeline orchestrator.
/// The engines themselves internally hold `Arc`-wrapped recognizers /
/// extractors, so cloning them is a few atomic increments.
///
/// [`EngineInner`]: crate::pipeline::Engine
pub struct RunContext {
    /// Token to signal cancellation to all tasks.
    pub(crate) cancel: CancellationToken,
    /// Shared run-wide state: run ID, actor, registry, policies.
    pub(crate) shared: Arc<SharedData>,
    /// Pre-built extractor registry.
    pub(crate) extraction_engine: ExtractorRegistry,
    /// Pre-built recognizer registry. Always present; when no
    /// recognizers are registered the per-modality dispatch
    /// short-circuits on the empty list. Shared via `Arc` so
    /// per-document phases hold a cheap handle without cloning
    /// the underlying recognizer lists.
    pub(crate) recognizer_registry: Arc<RecognizerRegistry>,
    /// Server-wide redaction defaults. Per-plan `Redaction` fields
    /// fall back to these.
    pub(crate) redaction_config: RedactionConfig,
    /// Per-modality custom-anonymizer registries. Populated by
    /// deployment code at engine startup; empty when no custom
    /// operators are registered (only built-in redaction specs from
    /// policies are then resolvable).
    pub(crate) redaction_registries: RedactionRegistries,
    /// Optional limit on how many documents may process concurrently.
    pub(crate) concurrency: Option<NonZeroUsize>,
}

/// Bundle of the four toolkit-shaped engine resources a
/// [`RunContext`] borrows from the pipeline orchestrator. Passed as
/// one argument so [`RunContext::new`] stays narrow as new engine
/// resources land.
pub(crate) struct RunEngines {
    pub extraction_engine: ExtractorRegistry,
    pub recognizer_registry: Arc<RecognizerRegistry>,
    pub redaction_config: RedactionConfig,
    pub redaction_registries: RedactionRegistries,
}

impl RunContext {
    /// Construct a [`RunContext`] from its parts. Called once per
    /// pass by a per-subsystem orchestrator.
    pub(crate) fn new(
        cancel: CancellationToken,
        shared: Arc<SharedData>,
        engines: RunEngines,
        concurrency: Option<NonZeroUsize>,
    ) -> Self {
        let RunEngines {
            extraction_engine,
            recognizer_registry,
            redaction_config,
            redaction_registries,
        } = engines;
        Self {
            cancel,
            shared,
            extraction_engine,
            recognizer_registry,
            redaction_config,
            redaction_registries,
            concurrency,
        }
    }

    /// True when this run's cancellation token has fired.
    pub(crate) fn is_cancelled(&self) -> bool {
        self.cancel.is_cancelled()
    }

    /// Pre-built extraction engine borrowed by [`ExtractionPhase`].
    ///
    /// [`ExtractionPhase`]: crate::pipeline::ExtractionPhase
    pub(crate) fn extraction_engine(&self) -> &ExtractorRegistry {
        &self.extraction_engine
    }

    /// Pre-built recognizer registry borrowed by [`DetectionPhase`].
    ///
    /// [`DetectionPhase`]: crate::pipeline::DetectionPhase
    pub(crate) fn recognizer_registry(&self) -> &Arc<RecognizerRegistry> {
        &self.recognizer_registry
    }

    /// Server-wide redaction defaults the [`RedactionPhase`] reads.
    ///
    /// [`RedactionPhase`]: crate::pipeline::RedactionPhase
    pub(crate) fn redaction_config(&self) -> &RedactionConfig {
        &self.redaction_config
    }

    /// Per-modality custom-anonymizer registries the
    /// [`RedactionPhase`] consults for `Custom`-arm lookups.
    ///
    /// [`RedactionPhase`]: crate::pipeline::RedactionPhase
    pub(crate) fn redaction_registries(&self) -> &RedactionRegistries {
        &self.redaction_registries
    }

    /// Run-wide shared state (policies, registry, key provider).
    pub(crate) fn shared(&self) -> &Arc<SharedData> {
        &self.shared
    }

    /// Optional document-concurrency cap from `[engine.limits]`.
    pub(crate) fn concurrency(&self) -> Option<NonZeroUsize> {
        self.concurrency
    }
}
