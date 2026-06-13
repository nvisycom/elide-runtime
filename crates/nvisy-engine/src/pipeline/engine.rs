//! [`Engine`] — the main entry point for running detection and
//! redaction pipelines.
//!
//! The engine is a thin facade over two per-subsystem pipelines.
//! It owns shared infrastructure (registry, key provider,
//! recognizer / extractor registries) and routes
//! [`Engine::detect`] and [`Engine::redact`] into the matching
//! per-pass pipeline.
//!
//! See `ARCHITECTURE.md` in this directory for the contract.

use std::path::Path;
use std::sync::Arc;
use std::{fmt, mem};

use nvisy_core::Error;
use nvisy_toolkit::detection::RecognizerRegistry;
use nvisy_toolkit::extraction::ExtractorRegistry;
use tokio::sync::Mutex;
use tokio::task::JoinSet;
use uuid::Uuid;

use super::config::{ExtractionConfig, RuntimeConfig};
use super::detection::{
    DetectionEngineState, DetectionEntry, DetectionFilter, DetectionInput, DetectionPipeline,
    DetectionSnapshot, DetectionState,
};
use super::redaction::{
    RedactionEngineState, RedactionEntry, RedactionFilter, RedactionInput, RedactionPipeline,
    RedactionSnapshot, RedactionState,
};
use crate::phases::ingestion::encryption::SharedKeyProvider;
use crate::phases::redaction::RedactionRegistries;
use crate::pipeline::RedactionConfig;
use crate::policy::validate_policy_namespace;
use crate::registry::Registry;

/// Shared inner state for the engine, held behind an `Arc`.
pub(super) struct EngineInner {
    /// Configuration loaded once at engine startup.
    pub runtime_config: RuntimeConfig,
    /// Pre-built extractor registry, constructed once from
    /// `runtime_config.extraction` and shared across every pass.
    pub extraction_engine: Arc<ExtractorRegistry>,
    /// Pre-built recognizer registry, constructed once from
    /// `runtime_config.detection` and shared across every pass.
    pub recognizer_registry: Arc<RecognizerRegistry>,
    /// Server-wide redaction defaults shared across every pass.
    pub redaction_config: Arc<RedactionConfig>,
    /// Per-modality custom-anonymizer registries.
    pub redaction_registries: Arc<RedactionRegistries>,
    /// Content and context storage backend.
    pub registry: Registry,
    /// Encryption key provider for import/export decrypt/encrypt.
    pub key_provider: Option<SharedKeyProvider>,
    /// In-memory detection-pass tracker for [`Engine::detect`].
    /// Volatile; the registry is the durability boundary.
    pub detections: DetectionState,
    /// In-memory redaction-pass tracker for [`Engine::redact`].
    /// Volatile; the registry is the durability boundary.
    pub redactions: RedactionState,
    /// Background pipeline tasks spawned by [`Engine::detect`] and
    /// [`Engine::redact`].
    background_tasks: Mutex<JoinSet<()>>,
}

/// Detection and redaction pipeline engine.
///
/// Thin facade over shared infrastructure. State lives in
/// `Arc<EngineInner>` and is shared across clones.
///
/// Builder methods ([`with_key_provider`]) require exclusive access
/// via `Arc::get_mut` and must be called before the engine is cloned.
///
/// [`with_key_provider`]: Engine::with_key_provider
#[derive(Clone)]
pub struct Engine {
    pub(super) inner: Arc<EngineInner>,
}

impl fmt::Debug for Engine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Engine")
            .field("runtime_config", &self.inner.runtime_config)
            .finish()
    }
}

impl Engine {
    /// Open a new engine with the given configuration and data directory.
    ///
    /// Constructs the registry and run state. HTTP clients are now
    /// the responsibility of individual extraction ops, which build
    /// them per-call from `RuntimeConfig`. Async because future
    /// recognizer registrations may need to connect to externalized
    /// inference services on first use.
    ///
    /// # Errors
    ///
    /// Returns an error if the registry database cannot be opened.
    pub async fn open(data_dir: impl AsRef<Path>, config: RuntimeConfig) -> Result<Self, Error> {
        let registry = Registry::open(data_dir.as_ref())?;
        let extraction_engine = Arc::new(
            config
                .extraction
                .as_ref()
                .map(ExtractionConfig::build)
                .transpose()?
                .unwrap_or_default(),
        );
        let recognizer_registry = Arc::new(match config.detection.as_ref() {
            Some(section) => section.build()?,
            None => RecognizerRegistry::default(),
        });
        let redaction_config = Arc::new(config.redaction.clone().unwrap_or_default());
        let redaction_registries = Arc::new(RedactionRegistries::default());

        Ok(Self {
            inner: Arc::new(EngineInner {
                runtime_config: config,
                extraction_engine,
                recognizer_registry,
                redaction_config,
                redaction_registries,
                registry,
                key_provider: None,
                detections: DetectionState::new(),
                redactions: RedactionState::new(),
                background_tasks: Mutex::new(JoinSet::new()),
            }),
        })
    }

    /// Create a temporary engine backed by a [`tempfile`] directory.
    ///
    /// Uses default configuration. The directory is deleted when the
    /// returned [`TempDir`] is dropped: keep it alive for the duration
    /// of the test.
    ///
    /// # Panics
    ///
    /// Panics if the temp directory or registry cannot be created.
    ///
    /// [`tempfile`]: https://docs.rs/tempfile
    /// [`TempDir`]: tempfile::TempDir
    #[cfg(any(test, feature = "test-utils"))]
    pub async fn temp() -> (Self, tempfile::TempDir) {
        let dir = tempfile::tempdir().expect("failed to create temp directory");
        let engine = Self::open(dir.path(), RuntimeConfig::default())
            .await
            .expect("failed to open engine in temp directory");
        (engine, dir)
    }

    /// Set the key provider for encryption/decryption operations.
    ///
    /// # Panics
    ///
    /// Panics if the engine has already been cloned (Arc is shared).
    pub fn with_key_provider(mut self, provider: SharedKeyProvider) -> Self {
        Arc::get_mut(&mut self.inner)
            .expect("engine must not be shared during construction")
            .key_provider = Some(provider);
        self
    }

    /// Install the per-modality custom-anonymizer registries the
    /// redaction phase consults for `Custom`-arm operator lookups.
    ///
    /// Replaces whatever registries the engine was constructed with
    /// (an empty default). Deployments call this once at startup
    /// after building their `RedactionRegistries`.
    ///
    /// # Panics
    ///
    /// Panics if the engine has already been cloned (Arc is shared).
    pub fn with_redaction_registries(mut self, registries: RedactionRegistries) -> Self {
        Arc::get_mut(&mut self.inner)
            .expect("engine must not be shared during construction")
            .redaction_registries = Arc::new(registries);
        self
    }

    /// Returns the base runtime configuration before per-request overrides.
    pub fn config(&self) -> &RuntimeConfig {
        &self.inner.runtime_config
    }

    /// Returns the content and context registry.
    pub fn registry(&self) -> &Registry {
        &self.inner.registry
    }

    /// Returns the base data directory path backing the registry.
    pub fn data_dir(&self) -> &Path {
        self.inner.registry.base_dir()
    }

    /// Construct a new detection pipeline bound to this engine's
    /// shared state.
    fn detection_pipeline(&self) -> DetectionPipeline {
        DetectionPipeline::new(
            self.inner.registry.clone(),
            self.inner.key_provider.clone(),
            self.inner.detections.clone(),
            self.inner.runtime_config.clone(),
            DetectionEngineState {
                extraction_engine: Arc::clone(&self.inner.extraction_engine),
                recognizer_registry: Arc::clone(&self.inner.recognizer_registry),
                redaction_config: Arc::clone(&self.inner.redaction_config),
                redaction_registries: Arc::clone(&self.inner.redaction_registries),
            },
        )
    }

    /// Submit a detection pass for background execution.
    ///
    /// Registers the pass synchronously as `Pending`, then spawns
    /// execution on a background task. Returns the detection id
    /// immediately. Poll [`Engine::get_detection`] for status and
    /// results.
    ///
    /// # Errors
    ///
    /// Currently infallible at submit time — failures during the
    /// pass itself are recorded on the in-memory state and
    /// observable via [`Engine::get_detection`]. The `Result`
    /// return reserves space for future synchronous validation.
    pub async fn detect(&self, input: DetectionInput) -> Result<Uuid, Error> {
        validate_policy_namespace(&input.policies)?;

        let pipeline = self.detection_pipeline();
        let detection_id = pipeline.id();
        pipeline.register_pending(&input).await;

        self.inner.background_tasks.lock().await.spawn(async move {
            if let Err(e) = pipeline.execute(input).await {
                tracing::error!(
                    %detection_id,
                    error = %e,
                    "detection pass failed",
                );
            }
        });
        Ok(detection_id)
    }

    /// Look up a detection-pass snapshot.
    ///
    /// Reads the in-memory state first; falls back to the
    /// persisted [`DetectionResult`] on disk when the in-memory
    /// record is absent (which happens after a process restart
    /// or after the in-memory record has been deleted but the
    /// persisted blob retained).
    ///
    /// # Errors
    ///
    /// [`nvisy_core::ErrorKind::NotFound`] when neither the
    /// in-memory state nor the registry has a matching entry
    /// for this `(actor_id, id)`.
    ///
    /// [`DetectionResult`]: super::detection::DetectionResult
    pub async fn get_detection(
        &self,
        actor_id: Uuid,
        id: Uuid,
    ) -> Result<DetectionSnapshot, Error> {
        match self.inner.detections.snapshot(actor_id, id).await {
            Ok(snap) => Ok(snap),
            Err(e) if e.kind() == nvisy_core::ErrorKind::NotFound => {
                // Try the registry. Successful load produces a
                // synthetic snapshot at Succeeded status with
                // hydrated audits but no fresh timestamps —
                // historical metadata is lost on restart.
                match self.inner.registry.load_detection(actor_id, id).await {
                    Ok(result) => Ok(DetectionSnapshot {
                        id: result.id,
                        actor_id: result.actor_id,
                        status: super::detection::DetectionStatus::Succeeded,
                        created_at: jiff::Timestamp::now(),
                        started_at: None,
                        completed_at: Some(jiff::Timestamp::now()),
                        result: Some(result),
                        error: None,
                    }),
                    Err(_) => Err(e),
                }
            }
            Err(e) => Err(e),
        }
    }

    /// List the actor's detection passes.
    pub async fn list_detections(
        &self,
        actor_id: Uuid,
        filter: DetectionFilter,
    ) -> Vec<DetectionEntry> {
        self.inner.detections.list(actor_id, filter).await
    }

    /// Cancel an in-progress detection.
    ///
    /// # Errors
    ///
    /// - [`nvisy_core::ErrorKind::NotFound`] when missing or
    ///   wrong actor.
    /// - [`nvisy_core::ErrorKind::Validation`] when terminal.
    pub async fn cancel_detection(&self, actor_id: Uuid, id: Uuid) -> Result<(), Error> {
        self.inner.detections.cancel(actor_id, id).await
    }

    /// Delete a finished detection. Cascades to the registry so
    /// the persisted result is removed atomically with the
    /// in-memory entry.
    ///
    /// # Errors
    ///
    /// - [`nvisy_core::ErrorKind::NotFound`] when missing or
    ///   wrong actor.
    /// - [`nvisy_core::ErrorKind::Validation`] when still active.
    pub async fn delete_detection(&self, actor_id: Uuid, id: Uuid) -> Result<(), Error> {
        self.inner.detections.delete(actor_id, id).await?;
        // In-memory delete succeeded; remove from disk. A failure
        // here is logged but not propagated — the in-memory
        // delete is the user-observable success criterion. A
        // background reconciliation job (future work) cleans up
        // orphaned blobs.
        if let Err(e) = self.inner.registry.unregister_detection(actor_id, id).await {
            tracing::warn!(
                detection_id = %id,
                error = %e,
                "failed to remove persisted detection result",
            );
        }
        Ok(())
    }

    /// Delete every finished detection for the actor. Active
    /// passes are preserved. Returns the number of entries
    /// removed. Cascades each deletion to the registry; cascade
    /// failures are logged but don't unblock the result count.
    pub async fn delete_all_detections(&self, actor_id: Uuid) -> usize {
        let removed = self.inner.detections.delete_all(actor_id).await;
        for id in &removed {
            if let Err(e) = self
                .inner
                .registry
                .unregister_detection(actor_id, *id)
                .await
            {
                tracing::warn!(
                    detection_id = %id,
                    error = %e,
                    "failed to remove persisted detection result during bulk delete",
                );
            }
        }
        removed.len()
    }

    /// Construct a new redaction pipeline bound to this engine's
    /// shared state.
    fn redaction_pipeline(&self) -> RedactionPipeline {
        RedactionPipeline::new(
            self.inner.registry.clone(),
            self.inner.key_provider.clone(),
            self.inner.detections.clone(),
            self.inner.redactions.clone(),
            self.inner.runtime_config.clone(),
            RedactionEngineState {
                extraction_engine: Arc::clone(&self.inner.extraction_engine),
                recognizer_registry: Arc::clone(&self.inner.recognizer_registry),
                redaction_config: Arc::clone(&self.inner.redaction_config),
                redaction_registries: Arc::clone(&self.inner.redaction_registries),
            },
        )
    }

    /// Submit a redaction pass for background execution.
    ///
    /// Registers the pass synchronously as `Pending`, then spawns
    /// execution on a background task. Returns the redaction id
    /// immediately. Poll [`Engine::get_redaction`] for status.
    ///
    /// Validation that runs synchronously before the task
    /// spawns: nothing. The detection lookup, override
    /// validation, and override application happen inside the
    /// background task so they're observable via `get_redaction`
    /// rather than returned from `redact`. This keeps the
    /// submission shape symmetric with [`Engine::detect`].
    ///
    /// # Errors
    ///
    /// Currently infallible at submit time. Pass-level failures
    /// (detection not found, override validation, override
    /// application, document errors) are recorded on the
    /// in-memory state and observable via
    /// [`Engine::get_redaction`].
    pub async fn redact(&self, input: RedactionInput) -> Result<Uuid, Error> {
        let pipeline = self.redaction_pipeline();
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
    /// [`nvisy_core::ErrorKind::NotFound`] when neither store
    /// has a matching entry.
    pub async fn get_redaction(
        &self,
        actor_id: Uuid,
        id: Uuid,
    ) -> Result<RedactionSnapshot, Error> {
        match self.inner.redactions.snapshot(actor_id, id).await {
            Ok(snap) => Ok(snap),
            Err(e) if e.kind() == nvisy_core::ErrorKind::NotFound => {
                match self.inner.registry.load_redaction(actor_id, id).await {
                    Ok(result) => Ok(RedactionSnapshot {
                        id: result.id,
                        detection_id: result.detection_id,
                        actor_id: result.actor_id,
                        status: super::redaction::RedactionStatus::Succeeded,
                        created_at: jiff::Timestamp::now(),
                        started_at: None,
                        completed_at: Some(jiff::Timestamp::now()),
                        result: Some(result),
                        error: None,
                    }),
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
    /// - [`nvisy_core::ErrorKind::NotFound`] when missing or
    ///   wrong actor.
    /// - [`nvisy_core::ErrorKind::Validation`] when terminal.
    pub async fn cancel_redaction(&self, actor_id: Uuid, id: Uuid) -> Result<(), Error> {
        self.inner.redactions.cancel(actor_id, id).await
    }

    /// Delete a finished redaction. Cascades to the registry.
    ///
    /// # Errors
    ///
    /// - [`nvisy_core::ErrorKind::NotFound`] when missing or
    ///   wrong actor.
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
    /// removed. Cascade failures are logged but don't unblock
    /// the result count.
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

    /// Wait for all background pipeline tasks to complete.
    ///
    /// Call during graceful shutdown to ensure in-flight runs finish
    /// and persist their results before the process exits.
    pub async fn shutdown(&self) {
        let tasks = mem::take(&mut *self.inner.background_tasks.lock().await);
        tasks.join_all().await;
    }
}
