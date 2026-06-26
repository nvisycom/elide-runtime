//! Application state and dependency injection.
//!
//! [`ServiceState`] wraps the single [`EngineHandle`] every
//! handler needs plus the deployment's default
//! [`AnalyzerParams`]. Cheaply cloneable (the engine handle is
//! `Arc`-backed and the analyzer spec is shared via `Arc`), so
//! per-HTTP-request handlers get a clone via `FromRef` without
//! coordinating lifetime.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use nvisy_core::Result;
use nvisy_core::plan::AnalyzerParams;
use nvisy_engine::EngineHandle;

/// Shared application state threaded through every handler.
#[must_use = "state does nothing unless you use it"]
#[derive(Clone)]
pub struct ServiceState {
    engine: EngineHandle,
    data_dir: PathBuf,
    analyzer_default: Arc<AnalyzerParams>,
}

impl ServiceState {
    /// Open the engine database under `data_dir`. The deployment's
    /// default analyzer is whatever the caller passes — typically
    /// the `[analyzer]` section of `Nvisy.toml`, or
    /// [`AnalyzerParams::default()`] when the section is absent
    /// (degenerate: no recognizers, no enrichers — runs that omit
    /// `analyzer` overrides will detect nothing).
    pub async fn new(data_dir: PathBuf, analyzer_default: AnalyzerParams) -> Result<Self> {
        let engine = EngineHandle::open(&data_dir)?;
        Ok(Self {
            engine,
            data_dir,
            analyzer_default: Arc::new(analyzer_default),
        })
    }

    /// The engine handle every handler reaches for.
    pub fn engine(&self) -> &EngineHandle {
        &self.engine
    }

    /// The data directory the engine database lives under.
    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    /// The deployment's default [`AnalyzerParams`]. Requests that
    /// inherit any analyzer field resolve against this.
    pub fn analyzer_default(&self) -> &AnalyzerParams {
        &self.analyzer_default
    }
}

impl axum::extract::FromRef<ServiceState> for EngineHandle {
    fn from_ref(state: &ServiceState) -> Self {
        state.engine.clone()
    }
}
