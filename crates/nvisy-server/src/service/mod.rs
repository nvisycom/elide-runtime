//! Application state and dependency injection.
//!
//! Split into two layers:
//!
//! - [`ServiceState`] is the clonable per-handler state — the
//!   `Engine` handle (Arc-backed) plus the shared analyzer
//!   default. Every handler holds a clone.
//! - [`ServiceRuntime`] is the outer, non-clonable owner: it
//!   holds the [`ServiceState`] plus long-lived background
//!   handles (the retention sweeper) that need orderly
//!   shutdown. The server binary constructs a `ServiceRuntime`,
//!   pulls the `ServiceState` for the router, and awaits
//!   [`ServiceRuntime::stop`] after `axum::serve` returns.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use nvisy_core::Result;
use nvisy_core::llm::LlmConfig;
use nvisy_core::ner::NerConfig;
use nvisy_engine::Engine;
use nvisy_engine::retention::SweeperHandle;
use nvisy_schema::plan::AnalyzerParams;

/// Default sweeper cadence when the caller doesn't specify.
/// Five minutes is small enough that a `ZeroRetention` policy
/// still deletes promptly after apply, and large enough that
/// the fjall full-scan on `list_due_retention` isn't a hot
/// path.
pub const DEFAULT_SWEEPER_INTERVAL: Duration = Duration::from_secs(300);

/// Shared application state threaded through every handler.
#[must_use = "state does nothing unless you use it"]
#[derive(Clone)]
pub struct ServiceState {
    engine: Engine,
    data_dir: PathBuf,
    analyzer_default: Arc<AnalyzerParams>,
}

impl ServiceState {
    /// The engine handle every handler reaches for.
    pub fn engine(&self) -> &Engine {
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

impl axum::extract::FromRef<ServiceState> for Engine {
    fn from_ref(state: &ServiceState) -> Self {
        state.engine.clone()
    }
}

/// Runtime owner: opens the engine, reaps crash-orphaned
/// active-file refs, starts the retention sweeper, and hands
/// out the [`ServiceState`] for the router. Non-clonable so
/// the sweeper handle has a single owner responsible for
/// [`stop`](Self::stop) on shutdown.
#[must_use = "runtime does nothing unless you use it"]
pub struct ServiceRuntime {
    state: ServiceState,
    sweeper: SweeperHandle,
}

impl ServiceRuntime {
    /// Open the engine database under `data_dir`, reap
    /// orphaned active-file references from any prior crash,
    /// and start the retention sweeper at `sweep_interval` (or
    /// [`DEFAULT_SWEEPER_INTERVAL`] when `None`).
    ///
    /// The deployment's default analyzer is whatever the caller
    /// passes — typically the `[analyzer]` section of
    /// `Nvisy.toml`, or [`AnalyzerParams::default()`] when the
    /// section is absent (degenerate: no recognizers, no
    /// enrichers — runs that omit `analyzer` overrides will
    /// detect nothing).
    pub async fn new(
        data_dir: PathBuf,
        analyzer_default: AnalyzerParams,
        ner: NerConfig,
        llm: LlmConfig,
        sweep_interval: Option<Duration>,
    ) -> Result<Self> {
        let engine = Engine::open(&data_dir)?.with_ner(ner).with_llm(llm);

        // Reap orphan active-file refs from crashes. Runs are
        // the source of truth for whether a file is still
        // referenced; any ref whose run is missing or terminal
        // is stale.
        let reaped = engine.reap_orphan_active_refs().await?;
        if reaped > 0 {
            tracing::info!(
                target: "nvisy_server::runtime",
                reaped,
                "reaped orphan active-file references at startup",
            );
        }

        let sweeper = engine.start_sweeper(sweep_interval.unwrap_or(DEFAULT_SWEEPER_INTERVAL));

        let state = ServiceState {
            engine,
            data_dir,
            analyzer_default: Arc::new(analyzer_default),
        };
        Ok(Self { state, sweeper })
    }

    /// Clone the [`ServiceState`] for handler wiring. The
    /// runtime keeps its own copy of the engine handle through
    /// the state clone; both point at the same `Arc`-backed
    /// registry.
    pub fn state(&self) -> ServiceState {
        self.state.clone()
    }

    /// Consume the runtime: stop the sweeper and await its
    /// task. Called by the server binary after `axum::serve`
    /// returns from graceful shutdown.
    pub async fn stop(self) {
        self.sweeper.stop().await;
    }
}
