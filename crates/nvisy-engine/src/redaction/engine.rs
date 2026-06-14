//! [`RedactionEngine`] — entry point for running redaction passes.
//!
//! Constructed from a paired [`DetectionEngine`]: the redaction
//! engine borrows the detection engine's [`Registry`], runtime
//! config, optional key provider, and an in-memory
//! [`DetectionState`] read-handle so the redact path can resolve a
//! prior detection by id without crossing the persistent
//! registry. Owns the redaction-side resources independently
//! (redaction config, custom-anonymizer registries, in-memory
//! redaction-pass tracker, background task set).
//!
//! [`DetectionEngine`]: super::super::detection::DetectionEngine
//! [`Registry`]: crate::registry::Registry
//! [`DetectionState`]: super::super::detection::DetectionState

use std::sync::Arc;
use std::{fmt, mem};

use nvisy_core::Error;
use tokio::sync::Mutex;
use tokio::task::JoinSet;
use uuid::Uuid;

use super::pipeline::{RedactionEngineState, RedactionPipeline};
use super::result::RedactionResult;
use super::state::RedactionState;
use super::status::RedactionStatus;
use super::{RedactionEntry, RedactionFilter, RedactionInput, RedactionSnapshot};
use crate::core::ingestion::encryption::SharedKeyProvider;
use crate::redaction::phases::RedactionRegistries;
use crate::core::RuntimeConfig;
use crate::redaction::RedactionConfig;
use crate::detection::{DetectionEngine, DetectionState};
use crate::registry::Registry;

/// Shared inner state for a [`RedactionEngine`], held behind an
/// `Arc`. Cloning the engine is a refcount bump on this inner.
pub(super) struct RedactionInner {
    pub runtime_config: RuntimeConfig,
    pub registry: Registry,
    pub key_provider: Option<SharedKeyProvider>,
    pub redaction_config: Arc<RedactionConfig>,
    pub redaction_registries: Arc<RedactionRegistries>,
    /// Read-handle to the paired [`DetectionEngine`][de]'s in-memory
    /// detection state. The redact path's handoff step resolves a
    /// prior detection-id through this without crossing the
    /// persistent registry.
    ///
    /// [de]: super::super::detection::DetectionEngine
    pub detections: DetectionState,
    /// In-memory redaction-pass tracker.
    pub redactions: RedactionState,
    /// Background pipeline tasks spawned by [`RedactionEngine::redact`].
    pub background_tasks: Mutex<JoinSet<()>>,
}

/// Redaction pipeline engine.
///
/// Thin facade over `Arc<RedactionInner>`; cloning is cheap.
/// Builder methods ([`with_redaction_registries`]) require
/// exclusive access via `Arc::get_mut` and must be called before
/// the engine is cloned.
///
/// [`with_redaction_registries`]: RedactionEngine::with_redaction_registries
#[derive(Clone)]
pub struct RedactionEngine {
    pub(super) inner: Arc<RedactionInner>,
}

impl fmt::Debug for RedactionEngine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RedactionEngine")
            .field("runtime_config", &self.inner.runtime_config)
            .finish()
    }
}

impl RedactionEngine {
    /// Build a redaction engine paired with `detection`.
    ///
    /// Shares the detection engine's [`Registry`], runtime
    /// configuration, optional key provider, and in-memory
    /// [`DetectionState`] (so the redact handoff path is a fast
    /// in-memory lookup, not a disk round-trip). Owns the
    /// redaction-side state (config, registries, in-memory
    /// tracker, background tasks) independently — calling
    /// [`shutdown`] on this engine drains only the redaction
    /// background tasks.
    ///
    /// [`Registry`]: crate::registry::Registry
    /// [`DetectionState`]: crate::detection::DetectionState
    /// [`shutdown`]: Self::shutdown
    pub fn from_detection(detection: &DetectionEngine) -> Self {
        let redaction_config = Arc::new(
            detection
                .config()
                .redaction
                .clone()
                .unwrap_or_default(),
        );
        Self {
            inner: Arc::new(RedactionInner {
                runtime_config: detection.config().clone(),
                registry: detection.registry().clone(),
                key_provider: detection.key_provider().cloned(),
                redaction_config,
                redaction_registries: Arc::new(RedactionRegistries::default()),
                detections: detection.detections().clone(),
                redactions: RedactionState::new(),
                background_tasks: Mutex::new(JoinSet::new()),
            }),
        }
    }

    /// Install the per-modality custom-anonymizer registries the
    /// redaction phase consults for `Custom`-arm operator lookups.
    ///
    /// Replaces whatever registries the engine was constructed
    /// with (an empty default). Deployments call this once at
    /// startup after building their [`RedactionRegistries`].
    ///
    /// # Panics
    ///
    /// Panics if the engine has already been cloned (Arc is shared).
    pub fn with_redaction_registries(mut self, registries: RedactionRegistries) -> Self {
        Arc::get_mut(&mut self.inner)
            .expect("redaction engine must not be shared during construction")
            .redaction_registries = Arc::new(registries);
        self
    }

    /// Returns the base runtime configuration before per-request
    /// overrides.
    pub fn config(&self) -> &RuntimeConfig {
        &self.inner.runtime_config
    }

    /// Returns the content and audit registry.
    pub fn registry(&self) -> &Registry {
        &self.inner.registry
    }

    /// Construct a new redaction pipeline bound to this engine's
    /// shared state.
    fn pipeline(&self) -> RedactionPipeline {
        RedactionPipeline::new(
            self.inner.registry.clone(),
            self.inner.key_provider.clone(),
            self.inner.detections.clone(),
            self.inner.redactions.clone(),
            self.inner.runtime_config.clone(),
            RedactionEngineState {
                redaction_config: Arc::clone(&self.inner.redaction_config),
                redaction_registries: Arc::clone(&self.inner.redaction_registries),
            },
        )
    }

    /// Submit a redaction pass for background execution.
    ///
    /// Registers the pass synchronously as `Pending`, then spawns
    /// execution on a background task. Returns the redaction id
    /// immediately. Poll [`Self::get_redaction`] for status.
    ///
    /// Validation that runs synchronously before the task spawns:
    /// nothing. The detection lookup, override validation, and
    /// override application happen inside the background task so
    /// they're observable via `get_redaction` rather than returned
    /// from `redact`. This keeps the submission shape symmetric
    /// with [`DetectionEngine::detect`][de].
    ///
    /// # Errors
    ///
    /// Currently infallible at submit time. Pass-level failures
    /// (detection not found, override validation, override
    /// application, document errors) are recorded on the
    /// in-memory state and observable via
    /// [`Self::get_redaction`].
    ///
    /// [de]: super::super::detection::DetectionEngine::detect
    pub async fn redact(&self, input: RedactionInput) -> Result<Uuid, Error> {
        let pipeline = self.pipeline();
        let redaction_id = pipeline.id();
        pipeline.register_pending(&input).await;

        self.inner.background_tasks.lock().await.spawn(async move {
            if let Err(e) = pipeline.execute(input).await {
                tracing::error!(
                    %redaction_id,
                    error = %e,
                    "redaction pass failed",
                );
            }
        });
        Ok(redaction_id)
    }

    /// Look up a redaction-pass snapshot. Hydrates from the
    /// registry when the in-memory record is absent.
    ///
    /// # Errors
    ///
    /// [`nvisy_core::ErrorKind::NotFound`] when neither store has
    /// a matching entry.
    pub async fn get_redaction(
        &self,
        actor_id: Uuid,
        id: Uuid,
    ) -> Result<RedactionSnapshot, Error> {
        match self.inner.redactions.snapshot(actor_id, id).await {
            Ok(snap) => Ok(snap),
            Err(e) if e.kind() == nvisy_core::ErrorKind::NotFound => {
                match self.inner.registry.load_redaction(actor_id, id).await {
                    Ok(result) => Ok(hydrate_snapshot(result)),
                    Err(_) => Err(e),
                }
            }
            Err(e) => Err(e),
        }
    }

    /// List the actor's redaction passes.
    pub async fn list_redactions(
        &self,
        actor_id: Uuid,
        filter: RedactionFilter,
    ) -> Vec<RedactionEntry> {
        self.inner.redactions.list(actor_id, filter).await
    }

    /// Cancel an in-progress redaction.
    ///
    /// # Errors
    ///
    /// - [`nvisy_core::ErrorKind::NotFound`] when missing or wrong
    ///   actor.
    /// - [`nvisy_core::ErrorKind::Validation`] when terminal.
    pub async fn cancel_redaction(&self, actor_id: Uuid, id: Uuid) -> Result<(), Error> {
        self.inner.redactions.cancel(actor_id, id).await
    }

    /// Delete a finished redaction. Cascades to the registry.
    ///
    /// # Errors
    ///
    /// - [`nvisy_core::ErrorKind::NotFound`] when missing or wrong
    ///   actor.
    /// - [`nvisy_core::ErrorKind::Validation`] when still active.
    pub async fn delete_redaction(&self, actor_id: Uuid, id: Uuid) -> Result<(), Error> {
        self.inner.redactions.delete(actor_id, id).await?;
        if let Err(e) = self.inner.registry.unregister_redaction(actor_id, id).await {
            tracing::warn!(
                redaction_id = %id,
                error = %e,
                "failed to remove persisted redaction result",
            );
        }
        Ok(())
    }

    /// Delete every finished redaction for the actor. Active
    /// passes are preserved. Returns the number of entries
    /// removed. Cascade failures are logged but don't unblock the
    /// result count.
    pub async fn delete_all_redactions(&self, actor_id: Uuid) -> usize {
        let removed = self.inner.redactions.delete_all(actor_id).await;
        for id in &removed {
            if let Err(e) = self
                .inner
                .registry
                .unregister_redaction(actor_id, *id)
                .await
            {
                tracing::warn!(
                    redaction_id = %id,
                    error = %e,
                    "failed to remove persisted redaction result during bulk delete",
                );
            }
        }
        removed.len()
    }

    /// Wait for all background redaction tasks to complete.
    ///
    /// Call during graceful shutdown to ensure in-flight passes
    /// finish and persist their results before the process exits.
    pub async fn shutdown(&self) {
        let tasks = mem::take(&mut *self.inner.background_tasks.lock().await);
        tasks.join_all().await;
    }
}

/// Build a synthetic [`RedactionSnapshot`] from a hydrated
/// [`RedactionResult`] loaded from disk. Historical metadata
/// collapses to "now" so the snapshot is non-empty.
fn hydrate_snapshot(result: RedactionResult) -> RedactionSnapshot {
    RedactionSnapshot {
        id: result.id,
        detection_id: result.detection_id,
        actor_id: result.actor_id,
        status: RedactionStatus::Succeeded,
        created_at: jiff::Timestamp::now(),
        started_at: None,
        completed_at: Some(jiff::Timestamp::now()),
        result: Some(result),
        error: None,
    }
}
