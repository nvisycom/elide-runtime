//! Composition helpers for [`Healthcheck`]-implementing components.
//!
//! The trait + data shapes live in [`nvisy_core::health`]. This
//! module sits one layer up and aggregates: probe a set of
//! components concurrently, collect their reports.
//!
//! Roll-up status and per-probe timeouts are caller concerns — wrap
//! individual probes in [`tokio::time::timeout`] before passing
//! them in, and compute a `Healthy / Degraded / Unhealthy`
//! overall from the returned [`ComponentCheck`]s where the
//! report is rendered.

use futures::future::join_all;
use nvisy_core::health::{ComponentCheck, Healthcheck};

/// Probe every [`Healthcheck`] in `items` concurrently and collect
/// their reports.
///
/// Each item's `name()` is read once and paired with its
/// `healthcheck()` result. The output preserves the input order —
/// no sorting, no roll-up, no overall status. The caller decides
/// how to render the list (HTTP response body, CLI table,
/// dashboard) and whether to compute a roll-up.
pub async fn probe_all<'a, I>(items: I) -> Vec<ComponentCheck>
where
    I: IntoIterator<Item = &'a dyn Healthcheck>,
{
    let futures = items.into_iter().map(|item| async move {
        let name = item.name();
        let status = item.healthcheck().await;
        ComponentCheck { name, status }
    });
    join_all(futures).await
}
