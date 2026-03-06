//! Parallel processing context.
//!
//! All inputs are collected upfront and handed to the operation in a
//! single call. Each item is processed independently: order does not
//! matter and no state is carried between items. The [`SharedContext`]
//! provides access to run-wide state (policies, contexts, actor, etc.).

use std::future::Future;

use derive_more::{Deref, DerefMut};

use super::sequential::SequentialContext;
use super::shared::SharedContext;
use super::{OperationContext, private};

/// Batch processing context: all inputs at once, no inter-item state.
///
/// Wraps per-call data `T` together with a [`SharedContext`] that
/// provides run-wide state. `Deref<Target = T>` and `DerefMut` forward
/// to the inner data for ergonomic field access.
///
/// [`Operation::Input`]: crate::operation::Operation::Input
/// [`Operation::Output`]: crate::operation::Operation::Output
#[derive(Debug, Clone, Deref, DerefMut)]
pub struct ParallelContext<T = ()> {
    /// Run-wide shared state.
    pub shared: SharedContext,
    /// The data carried by this context.
    #[deref]
    #[deref_mut]
    pub data: T,
}

impl<T: Send + Sync + 'static> private::Sealed for ParallelContext<T> {}
impl<T: Send + Sync + 'static> OperationContext for ParallelContext<T> {}

impl<T> ParallelContext<T> {
    /// Create a new parallel context with the given data and shared state.
    pub fn new(data: T, shared: SharedContext) -> Self {
        Self { shared, data }
    }

    /// Transform the inner data, preserving the shared context.
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> ParallelContext<U> {
        ParallelContext {
            shared: self.shared,
            data: f(self.data),
        }
    }

    /// Async fallibly transform the inner data into a [`ParallelContext`].
    pub async fn parallel_map<U, E, Fut>(self, f: impl FnOnce(T) -> Fut) -> Result<ParallelContext<U>, E>
    where
        Fut: Future<Output = Result<U, E>>,
    {
        Ok(ParallelContext {
            shared: self.shared,
            data: f(self.data).await?,
        })
    }

    /// Async fallibly transform the inner data into a [`SequentialContext`].
    pub async fn sequential_map<U, E, Fut>(self, f: impl FnOnce(T) -> Fut) -> Result<SequentialContext<U>, E>
    where
        Fut: Future<Output = Result<U, E>>,
    {
        Ok(SequentialContext {
            shared: self.shared,
            data: f(self.data).await?,
        })
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
