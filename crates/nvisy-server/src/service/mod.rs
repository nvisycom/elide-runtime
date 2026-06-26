//! Application state and dependency injection.
//!
//! [`ServiceState`] wraps the single [`EngineHandle`] every
//! handler needs. Cheaply cloneable (the inner handle is
//! `Arc`-backed), so per-HTTP-request handlers get a clone via
//! `FromRef` without coordinating lifetime.

use std::path::{Path, PathBuf};

use nvisy_core::Result;
use nvisy_engine::EngineHandle;

/// Shared application state threaded through every handler.
#[must_use = "state does nothing unless you use it"]
#[derive(Clone)]
pub struct ServiceState {
    engine: EngineHandle,
    data_dir: PathBuf,
}

impl ServiceState {
    /// Open (or create) the engine database under `data_dir`.
    ///
    /// Async to leave room for future first-use side-effects
    /// (e.g. recovering an in-flight run on cold start); today
    /// it just opens the fjall database via
    /// [`EngineHandle::open`].
    ///
    /// # Errors
    ///
    /// Returns [`nvisy_core::Error`] when the registry database
    /// cannot be opened.
    pub async fn new(data_dir: PathBuf) -> Result<Self> {
        let engine = EngineHandle::open(&data_dir)?;
        Ok(Self { engine, data_dir })
    }

    /// The engine handle every handler reaches for.
    pub fn engine(&self) -> &EngineHandle {
        &self.engine
    }

    /// The data directory the engine database lives under.
    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }
}

impl axum::extract::FromRef<ServiceState> for EngineHandle {
    fn from_ref(state: &ServiceState) -> Self {
        state.engine.clone()
    }
}
