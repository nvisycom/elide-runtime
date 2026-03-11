//! Sequential processing context.
//!
//! The orchestrator feeds one input per invocation, allowing the
//! operation to accumulate internal state between calls (e.g. known
//! entities for NER coreference). The [`SharedContext`] provides access
//! to run-wide state (policies, contexts, actor, etc.).

use std::future::Future;

use derive_more::{Deref, DerefMut};

use super::parallel::ParallelContext;
use super::shared::SharedContext;
use super::{OperationContext, private};

/// One-at-a-time processing context: the operation may carry state
/// between invocations.
///
/// Wraps per-call data `T` together with a [`SharedContext`] that
/// provides run-wide state. `Deref<Target = T>` and `DerefMut` forward
/// to the inner data for ergonomic field access.
#[derive(Debug, Clone, Deref, DerefMut)]
pub struct SequentialContext<T = ()> {
    /// Run-wide shared state.
    pub shared: SharedContext,
    /// The data carried by this context.
    #[deref]
    #[deref_mut]
    pub data: T,
}

impl<T: Send + Sync + 'static> private::Sealed for SequentialContext<T> {}
impl<T: Send + Sync + 'static> OperationContext for SequentialContext<T> {}

impl<T> SequentialContext<T> {
    /// Create a new sequential context with the given data and shared state.
    #[inline]
    pub fn new(data: T, shared: SharedContext) -> Self {
        Self { shared, data }
    }

    /// Async fallibly transform the inner data, staying in [`SequentialContext`].
    pub async fn sequential_map<U, E, Fut>(
        self,
        f: impl FnOnce(T) -> Fut,
    ) -> Result<SequentialContext<U>, E>
    where
        Fut: Future<Output = Result<U, E>>,
    {
        Ok(SequentialContext {
            shared: self.shared,
            data: f(self.data).await?,
        })
    }

    /// Async fallibly transform the inner data into a [`ParallelContext`].
    pub async fn parallel_map<U, E, Fut>(
        self,
        f: impl FnOnce(T) -> Fut,
    ) -> Result<ParallelContext<U>, E>
    where
        Fut: Future<Output = Result<U, E>>,
    {
        Ok(ParallelContext {
            shared: self.shared,
            data: f(self.data).await?,
        })
    }

    /// Borrow the inner data.
    #[inline]
    pub fn inner(&self) -> &T {
        &self.data
    }

    /// Mutably borrow the inner data.
    #[inline]
    pub fn inner_mut(&mut self) -> &mut T {
        &mut self.data
    }

    /// Consume the context and return the inner data.
    #[inline]
    pub fn into_inner(self) -> T {
        self.data
    }
}
