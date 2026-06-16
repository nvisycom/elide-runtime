//! [`DetectionEngine`] — entry point for running detection passes.
//!
//! Owns the detection-side resources (extractor registry, detection
//! config template, in-memory detection-pass tracker) plus the
//! shared infrastructure both engines need (registry, runtime
//! config, optional key provider, background task set). Per-pass
//! recognizer registries are built fresh inside the pipeline from
//! the detection-config template plus the request's policy-supplied
//! label catalog.

use std::path::Path;
use std::sync::Arc;
use std::{fmt, mem};

use nvisy_core::Error;
use nvisy_toolkit::extraction::ExtractorRegistry;
use tokio::sync::Mutex;
use tokio::task::JoinSet;
use uuid::Uuid;

use super::pipeline::{DetectionEngineState, DetectionPipeline};
use super::result::DetectionResult;
use super::state::DetectionState;
use super::status::DetectionStatus;
use super::{DetectionEntry, DetectionFilter, DetectionInput, DetectionSnapshot};
use crate::core::RuntimeConfig;
use crate::core::ingestion::encryption::SharedKeyProvider;
use crate::detection::{DetectionConfig, ExtractionConfig};
use crate::registry::Registry;

/// Shared inner state for a [`DetectionEngine`], held behind an
/// `Arc`. Cloning the engine is a refcount bump on this inner.
pub(super) struct DetectionInner {
    /// Configuration loaded once at engine startup.
    pub runtime_config: RuntimeConfig,
    /// Content + audit registry. Shared with [`RedactionEngine`]
    /// when the two are paired through
    /// [`RedactionEngine::from_detection`][rfd].
    ///
    /// [rfd]: super::super::redaction::RedactionEngine::from_detection
    pub registry: Registry,
    /// Pre-built extractor registry, constructed once from
    /// `runtime_config.extraction` and shared across every pass.
    pub extraction_engine: Arc<ExtractorRegistry>,
    /// Detection config template. The per-request
    /// [`RecognizerRegistry`][rr] is built fresh inside the
    /// detection pipeline from this template plus the request's
    /// policy-supplied label catalog.
    ///
    /// [rr]: nvisy_toolkit::detection::RecognizerRegistry
    pub detection_config: Arc<DetectionConfig>,
    /// Encryption key provider for import decrypt.
    pub key_provider: Option<SharedKeyProvider>,
    /// In-memory detection-pass tracker. Cloned (refcount bump)
    /// into a paired [`RedactionEngine`][re] so its handoff path
    /// can read prior detections without crossing the registry.
    ///
    /// [re]: super::super::redaction::RedactionEngine
    pub detections: DetectionState,
    /// Background pipeline tasks spawned by [`DetectionEngine::detect`].
    pub background_tasks: Mutex<JoinSet<()>>,
}

/// Detection pipeline engine.
///
/// Thin facade over `Arc<DetectionInner>`; cloning is cheap.
/// Builder methods ([`with_key_provider`]) require exclusive
/// access via `Arc::get_mut` and must be called before the engine
/// is cloned.
///
/// [`with_key_provider`]: DetectionEngine::with_key_provider
#[derive(Clone)]
pub struct DetectionEngine {
    pub(super) inner: Arc<DetectionInner>,
}

impl fmt::Debug for DetectionEngine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DetectionEngine")
            .field("runtime_config", &self.inner.runtime_config)
            .finish()
    }
}

impl DetectionEngine {
    /// Open a new detection engine with the given configuration and
    /// data directory.
    ///
    /// Constructs the registry and the extractor registry. Async
    /// because future recognizer registrations may need to connect
    /// to externalized inference services on first use.
    ///
    /// # Errors
    ///
    /// Returns an error if the registry database cannot be opened
    /// or the extractor registry fails to construct from
    /// `config.extraction`.
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
        let detection_config = Arc::new(config.detection.clone().unwrap_or_default());

        Ok(Self {
            inner: Arc::new(DetectionInner {
                runtime_config: config,
                registry,
                extraction_engine,
                detection_config,
                key_provider: None,
                detections: DetectionState::new(),
                background_tasks: Mutex::new(JoinSet::new()),
            }),
        })
    }

    /// Create a temporary detection engine backed by a [`tempfile`]
    /// directory.
    ///
    /// Uses default configuration. The directory is deleted when
    /// the returned [`TempDir`] is dropped: keep it alive for the
    /// duration of the test.
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
            .expect("failed to open detection engine in temp directory");
        (engine, dir)
    }

    /// Set the key provider for encryption/decryption operations.
    ///
    /// # Panics
    ///
    /// Panics if the engine has already been cloned (Arc is shared).
    pub fn with_key_provider(mut self, provider: SharedKeyProvider) -> Self {
        Arc::get_mut(&mut self.inner)
            .expect("detection engine must not be shared during construction")
            .key_provider = Some(provider);
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

    /// Returns the optional key provider. [`RedactionEngine`] reads
    /// this through [`from_detection`][rfd] so both engines share
    /// the same decrypt/encrypt material without re-installing it.
    ///
    /// [`RedactionEngine`]: super::super::redaction::RedactionEngine
    /// [rfd]: super::super::redaction::RedactionEngine::from_detection
    pub fn key_provider(&self) -> Option<&SharedKeyProvider> {
        self.inner.key_provider.as_ref()
    }

    /// Returns the in-memory detection state. A paired
    /// [`RedactionEngine`][re] clones this handle in
    /// [`from_detection`][rfd] so its redact path can resolve a
    /// prior detection by id without touching the persistent
    /// registry.
    ///
    /// [re]: super::super::redaction::RedactionEngine
    /// [rfd]: super::super::redaction::RedactionEngine::from_detection
    pub fn detections(&self) -> &DetectionState {
        &self.inner.detections
    }

    /// Returns the base data directory path backing the registry.
    pub fn data_dir(&self) -> &Path {
        self.inner.registry.base_dir()
    }

    /// Construct a new detection pipeline bound to this engine's
    /// shared state.
    fn pipeline(&self) -> DetectionPipeline {
        DetectionPipeline::new(
            self.inner.registry.clone(),
            self.inner.key_provider.clone(),
            self.inner.detections.clone(),
            self.inner.runtime_config.clone(),
            DetectionEngineState {
                extraction_engine: Arc::clone(&self.inner.extraction_engine),
                detection_config: Arc::clone(&self.inner.detection_config),
            },
        )
    }

    /// Submit a detection pass for background execution.
    ///
    /// Validates the submitted request synchronously (policy/rule
    /// name uniqueness, label union with conflict detection,
    /// selector-label resolution against the unioned catalog,
    /// action validation). On success, registers the pass as
    /// `Pending`, spawns execution on a background task, and
    /// returns the detection id immediately. Poll
    /// [`Self::get_detection`] for status and results.
    ///
    /// # Errors
    ///
    /// Returns a validation error when the submitted request fails
    /// any of the four request-entry checks.
    pub async fn detect(&self, input: DetectionInput) -> Result<Uuid, Error> {
        let pipeline = self.pipeline();
        let detection_id = pipeline.id();
        let prepared = pipeline.register_pending(input).await?;

        self.inner.background_tasks.lock().await.spawn(async move {
            if let Err(e) = pipeline.execute(prepared).await {
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
    /// record is absent (which happens after a process restart or
    /// after the in-memory record has been deleted but the
    /// persisted blob retained).
    ///
    /// # Errors
    ///
    /// [`nvisy_core::ErrorKind::NotFound`] when neither the
    /// in-memory state nor the registry has a matching entry for
    /// this `(actor_id, id)`.
    pub async fn get_detection(
        &self,
        actor_id: Uuid,
        id: Uuid,
    ) -> Result<DetectionSnapshot, Error> {
        match self.inner.detections.snapshot(actor_id, id).await {
            Ok(snap) => Ok(snap),
            Err(e) if e.kind() == nvisy_core::ErrorKind::NotFound => {
                match self.inner.registry.load_detection(actor_id, id).await {
                    Ok(result) => Ok(hydrate_snapshot(result)),
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
    /// - [`nvisy_core::ErrorKind::NotFound`] when missing or wrong
    ///   actor.
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
    /// - [`nvisy_core::ErrorKind::NotFound`] when missing or wrong
    ///   actor.
    /// - [`nvisy_core::ErrorKind::Validation`] when still active.
    pub async fn delete_detection(&self, actor_id: Uuid, id: Uuid) -> Result<(), Error> {
        self.inner.detections.delete(actor_id, id).await?;
        // In-memory delete succeeded; remove from disk. A failure
        // here is logged but not propagated — the in-memory delete
        // is the user-observable success criterion. A background
        // reconciliation job (future work) cleans up orphaned
        // blobs.
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

    /// Wait for all background detection tasks to complete.
    ///
    /// Call during graceful shutdown to ensure in-flight passes
    /// finish and persist their results before the process exits.
    pub async fn shutdown(&self) {
        let tasks = mem::take(&mut *self.inner.background_tasks.lock().await);
        tasks.join_all().await;
    }
}

/// Build a synthetic [`DetectionSnapshot`] from a hydrated
/// [`DetectionResult`] loaded from disk. Historical metadata
/// (created_at, started_at) is lost on process restart — both
/// timestamps collapse to "now" so the snapshot is non-empty.
fn hydrate_snapshot(result: DetectionResult) -> DetectionSnapshot {
    DetectionSnapshot {
        id: result.id,
        actor_id: result.actor_id,
        status: DetectionStatus::Succeeded,
        created_at: jiff::Timestamp::now(),
        started_at: None,
        completed_at: Some(jiff::Timestamp::now()),
        result: Some(result),
        error: None,
    }
}
