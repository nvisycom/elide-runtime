//! Per-pass execution contexts: [`DetectionContext`] and
//! [`RedactionContext`].
//!
//! Each side carries only the engine resources it actually consumes
//! — detection needs the extractor and recognizer registries,
//! redaction needs the redaction config and per-modality
//! anonymizer registries. The fields they share — run-wide
//! [`SharedData`], cancellation, concurrency cap — are exposed
//! through the [`PhaseContext`] trait so modality-agnostic phases
//! (deduplication, validation) can borrow either context type
//! without duplication.
//!
//! Lives in `core/` because phases consume these types through
//! their `ctx` parameter — they're part of the phase contract
//! surface, not the orchestrator's private state.

use std::num::NonZeroUsize;
use std::sync::Arc;

use nvisy_toolkit::detection::RecognizerRegistry;
use nvisy_toolkit::extraction::ExtractorRegistry;
use tokio_util::sync::CancellationToken;

use super::SharedData;
use crate::redaction::phases::RedactionRegistries;
use crate::redaction::RedactionConfig;

/// Shared surface every phase reads from regardless of which side
/// (detection or redaction) it runs on. Implemented by both
/// [`DetectionContext`] and [`RedactionContext`]; modality-agnostic
/// phases (deduplication, validation) bound on this trait so they
/// can borrow either context type.
pub trait PhaseContext {
    /// Run-wide shared state (policies, registry, key provider,
    /// per-request entity-label catalog).
    fn shared(&self) -> &Arc<SharedData>;
    /// `true` when this run's cancellation token has fired.
    fn is_cancelled(&self) -> bool;
    /// Optional document-concurrency cap from `[engine.limits]`.
    fn concurrency(&self) -> Option<NonZeroUsize>;
}

/// Per-pass detection context. Detection-side phases (extraction,
/// detection, deduplication) consume it; redaction-side phases
/// never see it.
pub struct DetectionContext {
    pub(crate) cancel: CancellationToken,
    pub(crate) shared: Arc<SharedData>,
    pub(crate) extraction_engine: ExtractorRegistry,
    /// Per-request recognizer registry — built fresh from the
    /// engine-side detection-config template plus the request's
    /// label catalog.
    pub(crate) recognizer_registry: Arc<RecognizerRegistry>,
    pub(crate) concurrency: Option<NonZeroUsize>,
}

/// Engine resources the detection pipeline borrows when building a
/// [`DetectionContext`]. Bundled into one type so adding new
/// detection-side engines doesn't widen `DetectionContext::new`'s
/// signature.
pub(crate) struct DetectionEngines {
    pub extraction_engine: ExtractorRegistry,
    pub recognizer_registry: Arc<RecognizerRegistry>,
}

impl DetectionContext {
    /// Construct a [`DetectionContext`] from its parts. Called once
    /// per pass by the detection pipeline.
    pub(crate) fn new(
        cancel: CancellationToken,
        shared: Arc<SharedData>,
        engines: DetectionEngines,
        concurrency: Option<NonZeroUsize>,
    ) -> Self {
        let DetectionEngines {
            extraction_engine,
            recognizer_registry,
        } = engines;
        Self {
            cancel,
            shared,
            extraction_engine,
            recognizer_registry,
            concurrency,
        }
    }

    /// Pre-built extraction engine borrowed by [`ExtractionPhase`].
    ///
    /// [`ExtractionPhase`]: crate::detection::phases::extraction::ExtractionPhase
    pub(crate) fn extraction_engine(&self) -> &ExtractorRegistry {
        &self.extraction_engine
    }

    /// Per-request recognizer registry borrowed by
    /// [`DetectionPhase`].
    ///
    /// [`DetectionPhase`]: crate::detection::phases::detection::DetectionPhase
    pub(crate) fn recognizer_registry(&self) -> &Arc<RecognizerRegistry> {
        &self.recognizer_registry
    }
}

impl PhaseContext for DetectionContext {
    fn shared(&self) -> &Arc<SharedData> {
        &self.shared
    }

    fn is_cancelled(&self) -> bool {
        self.cancel.is_cancelled()
    }

    fn concurrency(&self) -> Option<NonZeroUsize> {
        self.concurrency
    }
}

/// Per-pass redaction context. Redaction-side phases (redaction,
/// validation) consume it; detection-side phases never see it.
pub struct RedactionContext {
    pub(crate) cancel: CancellationToken,
    pub(crate) shared: Arc<SharedData>,
    pub(crate) redaction_config: RedactionConfig,
    pub(crate) redaction_registries: RedactionRegistries,
    pub(crate) concurrency: Option<NonZeroUsize>,
}

/// Engine resources the redaction pipeline borrows when building a
/// [`RedactionContext`]. Bundled into one type so adding new
/// redaction-side engines doesn't widen `RedactionContext::new`'s
/// signature.
pub(crate) struct RedactionEngines {
    pub redaction_config: RedactionConfig,
    pub redaction_registries: RedactionRegistries,
}

impl RedactionContext {
    /// Construct a [`RedactionContext`] from its parts. Called once
    /// per pass by the redaction pipeline.
    pub(crate) fn new(
        cancel: CancellationToken,
        shared: Arc<SharedData>,
        engines: RedactionEngines,
        concurrency: Option<NonZeroUsize>,
    ) -> Self {
        let RedactionEngines {
            redaction_config,
            redaction_registries,
        } = engines;
        Self {
            cancel,
            shared,
            redaction_config,
            redaction_registries,
            concurrency,
        }
    }

    /// Server-wide redaction defaults the [`RedactionPhase`] reads.
    ///
    /// [`RedactionPhase`]: crate::redaction::phases::RedactionPhase
    pub(crate) fn redaction_config(&self) -> &RedactionConfig {
        &self.redaction_config
    }

    /// Per-modality custom-anonymizer registries the
    /// [`RedactionPhase`] consults for `Custom`-arm lookups.
    ///
    /// [`RedactionPhase`]: crate::redaction::phases::RedactionPhase
    pub(crate) fn redaction_registries(&self) -> &RedactionRegistries {
        &self.redaction_registries
    }
}

impl PhaseContext for RedactionContext {
    fn shared(&self) -> &Arc<SharedData> {
        &self.shared
    }

    fn is_cancelled(&self) -> bool {
        self.cancel.is_cancelled()
    }

    fn concurrency(&self) -> Option<NonZeroUsize> {
        self.concurrency
    }
}
