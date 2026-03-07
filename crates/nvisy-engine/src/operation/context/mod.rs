//! Operation contexts: processing strategy and run-wide shared state.
//!
//! Every operation receives its input and produces its output wrapped in
//! a context type. The context serves two purposes:
//!
//! 1. **Processing strategy**: the concrete wrapper ([`ParallelContext`]
//!    or [`SequentialContext`]) tells the orchestrator *how* to invoke
//!    the operation (batch all inputs vs. feed one-by-one).
//!
//! 2. **Shared state**: both wrappers carry a [`SharedContext`], giving
//!    every operation cheap access to run-wide data (run id, actor,
//!    policies, reference-data contexts) without threading it through
//!    individual parameter structs.
//!
//! The [`OperationContext`] trait is **sealed**: only [`ParallelContext`]
//! and [`SequentialContext`] may implement it, so the orchestrator only
//! needs to handle two calling conventions.
//!
//! [`Operation::Input`]: crate::operation::Operation::Input
//! [`Operation::Output`]: crate::operation::Operation::Output

mod parallel;
mod sequential;
mod shared;

pub use parallel::ParallelContext;
pub use sequential::SequentialContext;
pub use shared::SharedContext;

pub(crate) mod private {
    pub trait Sealed {}
}

/// Marker trait for operation processing strategies.
///
/// This trait is sealed and cannot be implemented outside this crate.
pub trait OperationContext: private::Sealed + Send + Sync + 'static {}
