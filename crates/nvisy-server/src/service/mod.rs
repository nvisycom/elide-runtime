//! Application state and dependency injection.
//!
//! [`ServiceState`] holds both [`DetectionEngine`] and
//! [`RedactionEngine`]. The redaction engine is constructed from
//! the detection engine via
//! [`RedactionEngine::from_detection`][rfd] so they share the
//! same registry, runtime config, optional key provider, and an
//! in-memory `DetectionState` read-handle for the
//! detect→redact handoff.
//!
//! Individual handlers extract whichever engine they need through
//! `FromRef`.
//!
//! [rfd]: nvisy_engine::redaction::RedactionEngine::from_detection

use std::path::{Path, PathBuf};

use nvisy_core::Result;
use nvisy_engine::core::RuntimeConfig;
use nvisy_engine::detection::DetectionEngine;
use nvisy_engine::redaction::RedactionEngine;
use nvisy_engine::registry::Registry;

/// Shared application state threaded through all handlers.
#[must_use = "state does nothing unless you use it"]
#[derive(Clone)]
pub struct ServiceState {
    detection: DetectionEngine,
    redaction: RedactionEngine,
}

impl ServiceState {
    /// Creates a new service state from a resolved [`RuntimeConfig`]
    /// and data directory.
    ///
    /// Async because engine construction may download model
    /// artifacts on first use. The redaction engine is constructed
    /// from the detection engine so they share the underlying
    /// registry, runtime config, and key provider.
    ///
    /// # Errors
    ///
    /// Returns an error if the registry database cannot be opened.
    pub async fn new(config: RuntimeConfig, data_dir: PathBuf) -> Result<Self> {
        let detection = DetectionEngine::open(data_dir, config).await?;
        let redaction = RedactionEngine::from_detection(&detection);
        Ok(Self {
            detection,
            redaction,
        })
    }

    /// Returns the data directory path.
    pub fn data_dir(&self) -> &Path {
        self.detection.data_dir()
    }
}

macro_rules! impl_di {
    ($($extract:expr => $t:ty),+ $(,)?) => {$(
        impl axum::extract::FromRef<ServiceState> for $t {
            fn from_ref(state: &ServiceState) -> Self {
                $extract(state)
            }
        }
    )+};
}

impl_di!(
    |s: &ServiceState| s.detection.clone() => DetectionEngine,
    |s: &ServiceState| s.redaction.clone() => RedactionEngine,
    |s: &ServiceState| s.detection.registry().clone() => Registry,
);
