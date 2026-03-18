//! Compiled runtime policy types.
//!
//! These types convert user-facing config types from [`crate::graph::policy`]
//! into runtime representations with pre-computed [`Duration`] values and
//! async execution helpers.
//!
//! [`Duration`]: std::time::Duration
//!
//! [`CompiledTimeoutPolicy`] wraps the entire node execution with a
//! deadline. [`CompiledRetryPolicy`] wraps each individual item
//! processed by the node (e.g. each content import).

mod retry;
mod timeout;

pub use self::retry::CompiledRetryPolicy;
pub use self::timeout::CompiledTimeoutPolicy;
