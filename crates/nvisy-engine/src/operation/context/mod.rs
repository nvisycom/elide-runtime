//! Processing-strategy markers for operations.
//!
//! An operation encodes its processing model via the [`OperationContext`]
//! bound on [`Operation::Input`] and [`Operation::Output`]. The
//! orchestrator inspects the concrete wrapper at the type level to decide
//! whether to batch all inputs upfront or iterate one-by-one.
//!
//! The trait is **sealed**: only [`ParallelContext`] and
//! [`SequentialContext`] may implement it. This guarantees the
//! orchestrator only needs to handle two calling conventions.
//!
//! [`Operation::Input`]: crate::operation::Operation::Input
//! [`Operation::Output`]: crate::operation::Operation::Output

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
