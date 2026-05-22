//! Pipeline policy configuration types: per-phase timeouts and
//! concurrency limits.

mod concurrency;
mod phase;
mod timeout;

pub use self::concurrency::ConcurrencyPolicy;
pub use self::phase::PhasePolicy;
pub use self::timeout::{TimeoutBehavior, TimeoutPolicy};
