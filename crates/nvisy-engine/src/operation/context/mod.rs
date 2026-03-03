//! Processing-strategy markers for operations.
//!
//! An operation advertises its processing model via an
//! [`OperationContext`] associated type. The orchestrator inspects
//! the concrete context at the type level to decide whether to batch
//! all inputs upfront or iterate one-by-one.
//!
//! The trait is **sealed** — only [`ParallelContext`] and
//! [`SequentialContext`] may implement it. This guarantees the
//! orchestrator only needs to handle two calling conventions.

mod parallel;
mod sequential;

pub use parallel::ParallelContext;
pub use sequential::SequentialContext;

pub(crate) mod private {
    pub trait Sealed {}
}

/// Marker trait for operation processing strategies.
///
/// This trait is sealed and cannot be implemented outside this crate.
pub trait OperationContext: private::Sealed + Send + Sync + 'static {}
