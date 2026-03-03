//! Parallel processing context.
//!
//! All inputs are collected upfront and processed independently in a
//! single call.

use derive_more::{Deref, DerefMut, From};

use super::private;
use super::OperationContext;

/// All inputs are collected upfront and processed independently.
///
/// The orchestrator gathers every input, then passes them to
/// [`Operation::call`](crate::operation::Operation::call) in a single call.
#[derive(Debug, Clone, Deref, DerefMut, From)]
pub struct ParallelContext<T = ()> {
    /// The data carried by this context.
    pub data: T,
}

impl<T: Send + Sync + 'static> private::Sealed for ParallelContext<T> {}
impl<T: Send + Sync + 'static> OperationContext for ParallelContext<T> {}

impl Default for ParallelContext<()> {
    fn default() -> Self {
        Self { data: () }
    }
}

impl<T> ParallelContext<T> {
    /// Create a new parallel context with the given data.
    pub fn new(data: T) -> Self {
        Self { data }
    }

    /// Transform the inner data, preserving the context wrapper.
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> ParallelContext<U> {
        ParallelContext { data: f(self.data) }
    }

    /// Borrow the inner data.
    pub fn inner(&self) -> &T {
        &self.data
    }

    /// Mutably borrow the inner data.
    pub fn inner_mut(&mut self) -> &mut T {
        &mut self.data
    }

    /// Consume the context and return the inner data.
    pub fn into_inner(self) -> T {
        self.data
    }
}
