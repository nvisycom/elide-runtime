//! Operational status reporting.
//!
//! The transport-agnostic vocabulary every component (recognizer,
//! extractor, storage, backend) uses to report whether it's ready
//! to serve.
//!
//! - [`ServiceStatus`]: the three-state classification
//!   ([`Healthy`] / [`Degraded`] / [`Unhealthy`]).
//! - [`ComponentCheck`]: one named status report.
//! - [`Healthcheck`]: an async probe a component implements to
//!   report its own state. Probes should be cheap — no real work,
//!   just enough to confirm the component would respond to a real
//!   request.
//!
//! Composition helpers (probing multiple components concurrently,
//! computing a roll-up status, applying per-probe timeouts) live
//! at the layer that aggregates components — typically the
//! engine — to keep this module dependency-free.
//!
//! The HTTP response envelope (a `Health` body with a roll-up
//! status + per-component checks + timestamp) lives at the
//! transport layer that needs it.
//!
//! [`Healthy`]: ServiceStatus::Healthy
//! [`Degraded`]: ServiceStatus::Degraded
//! [`Unhealthy`]: ServiceStatus::Unhealthy

use std::borrow::Cow;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Operational status of a service or component.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ServiceStatus {
    /// Operating normally.
    #[default]
    Healthy,
    /// Operating with some issues but still functional.
    Degraded,
    /// Not operational.
    Unhealthy,
}

/// Status of a single named component.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ComponentCheck {
    /// Component name (e.g. `"filesystem"`, `"registry"`,
    /// `"ner-bento"`).
    pub name: Cow<'static, str>,
    /// Status of this component.
    pub status: ServiceStatus,
}

impl ComponentCheck {
    /// Construct a check with a static-borrowed name.
    pub fn new(name: &'static str, status: ServiceStatus) -> Self {
        Self {
            name: Cow::Borrowed(name),
            status,
        }
    }
}

/// Component that can report its own operational state.
///
/// Implementations should make probes **cheap**: a cached liveness
/// flag, a single round-trip to a backing service, a small ping —
/// not anything that does real work. Expensive checks block
/// `/health` and risk timing out under load. The caller composing
/// the report is responsible for applying per-probe timeouts (via
/// `tokio::time::timeout` or similar) — keep the trait itself
/// timeout-free.
///
/// Each implementor reports independently: a registry that
/// holds children should expose each child via its own
/// `&dyn Healthcheck`, not roll them up. Operators want
/// per-component visibility ("ocr-bento is degraded, ocr-tesseract
/// is healthy"), not aggregate status.
#[async_trait::async_trait]
pub trait Healthcheck: Send + Sync {
    /// Stable identifier for this component on the wire (e.g.
    /// `"ner-bento"`, `"ocr-tesseract"`, `"registry"`).
    fn name(&self) -> Cow<'static, str>;

    /// Probe operational state.
    async fn healthcheck(&self) -> ServiceStatus;
}
