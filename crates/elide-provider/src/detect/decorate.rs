//! Wrapping a backend in the decorators a component configures.
//!
//! Every configured backend is metered, so an audit says what each
//! call cost. A component may also cap how often it is called, and
//! how many calls may be in flight.
//!
//! The decorators are themselves backends — same request, same
//! response, same provenance — so they nest. The order is fixed and
//! matters:
//!
//! ```text
//! Metered<ConcurrencyLimited<RateLimited<BatchChunked<B>>>>
//! ```
//!
//! Metering outermost, so a call delayed or queued by a limiter is
//! still recorded with the wall time it actually took. Putting
//! `Metered` innermost would report only the time the backend spent
//! once admitted, hiding exactly the latency a limit introduces.
//!
//! The limits are optional and independent: a rate limit bounds
//! calls per window, a concurrency limit bounds calls in flight. A
//! slow backend can sit under its rate and still have a hundred
//! requests outstanding, which is why capping one does not cap the
//! other.
//!
//! `BatchChunked` bounds how much work one request carries. A hosted
//! service splits an oversized batch itself, so this is not what keeps
//! a large batch from being refused; what it keeps is the request from
//! having to *finish* inside one HTTP timeout. A whole document
//! coalesced into a single `call_batch` is one request that must
//! complete before the provider's deadline, and chunking makes it
//! several, each with its own.
//!
//! Innermost, so one oversized `call_batch` is one admission rather
//! than several. Outside the limiters each window would take its own
//! permit and its own token, turning a caller's single batch into a
//! burst against limits it never asked to spend — and the windows run
//! sequentially anyway, so there is nothing for a concurrency limit to
//! bound.

use std::sync::Arc;
use std::time::Duration;

use elide::backend::metrics::{Metered, UsageSink};
use elide::backend::ratelimit::{BatchChunked, ConcurrencyLimited, RateLimited};
use elide_deployment::RateLimit;

/// The decorator stack a component's configuration asks for.
///
/// Always `Metered`; the two limits are applied only when
/// configured, and a component with neither pays nothing beyond
/// the metering every backend already carries.
pub(crate) type Decorated<B> = Metered<ConcurrencyLimited<RateLimited<BatchChunked<B>>>>;

/// Wrap `backend` in the decorators `rate_limit` describes.
///
/// A component with no rate limit still gets the full type, with
/// both limiters set wide enough never to admit-block: that keeps
/// one concrete type per backend kind, which is what lets the
/// recognizer type parameter stay single-valued.
///
/// The cost is one uncontended semaphore acquire and one
/// token-bucket check per call — negligible, because a `call` is a
/// network round trip to a hosted model. Paying it buys one type
/// instead of two code paths that could diverge.
pub(crate) fn decorate<B>(
    backend: B,
    rate_limit: Option<&RateLimit>,
    sink: Arc<dyn UsageSink>,
) -> Decorated<B> {
    // `None` means unlimited, and the decorators have no such mode:
    // their constructors reject zero and admit at the rate given. So
    // an unlimited component is given a limit it cannot reach rather
    // than a second code path with a different type.
    let (refill, interval, burst, permits, batch) = match rate_limit {
        Some(limit) => (
            limit.requests,
            Duration::from_secs(u64::from(limit.interval_seconds)),
            limit.burst_or_default(),
            limit.max_concurrent.unwrap_or(UNLIMITED_PERMITS),
            limit.max_batch_size.unwrap_or(UNLIMITED_BATCH),
        ),
        None => (
            UNLIMITED_RATE,
            Duration::from_secs(1),
            UNLIMITED_RATE,
            UNLIMITED_PERMITS,
            UNLIMITED_BATCH,
        ),
    };

    let chunked = BatchChunked::new(backend, batch);
    let limited = RateLimited::new(chunked, refill, interval, burst);
    let bounded = ConcurrencyLimited::new(limited, permits);
    Metered::new(bounded, sink)
}

/// A per-second rate no deployment reaches, standing in for "no
/// rate limit".
///
/// The decorator has no unlimited mode and rejects zero, so an
/// unconfigured component needs a ceiling instead. A million calls
/// per second is far above any hosted backend's own quota, so the
/// bucket is never the binding constraint.
const UNLIMITED_RATE: u32 = 1_000_000;

/// A permit count no deployment reaches, standing in for "no
/// concurrency limit".
const UNLIMITED_PERMITS: usize = 1_000_000;

/// A batch size no caller reaches, standing in for "no batch limit".
///
/// `BatchChunked` has no unlimited mode and rejects zero, so a
/// provider with no batch cap is given a window wider than any batch
/// it will be handed; the decorator then passes every batch straight
/// through untouched.
const UNLIMITED_BATCH: usize = 1_000_000;
