//! Compiled runtime policies and execution helpers.
//!
//! The compiler-level [`RetryPolicy`](crate::compiler::RetryPolicy) and
//! [`TimeoutPolicy`](crate::compiler::TimeoutPolicy) are user-facing
//! configuration types. This module provides their compiled runtime
//! counterparts ([`CompiledRetryPolicy`], [`CompiledTimeoutPolicy`])
//! with `with_retry` and `with_timeout` methods.

mod retry;
mod timeout;

pub use self::retry::CompiledRetryPolicy;
pub use self::timeout::CompiledTimeoutPolicy;
