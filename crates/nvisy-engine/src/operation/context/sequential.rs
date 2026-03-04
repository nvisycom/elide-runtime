//! Sequential processing context.
//!
//! Inputs are processed one at a time, allowing the operation to carry
//! state between calls.

use derive_more::{Deref, DerefMut, From};

use super::private;
use super::OperationContext;

/// Inputs are processed one at a time; the operation carries state
/// between calls.
///
/// The orchestrator feeds one input per invocation, allowing the
/// operation to accumulate context (e.g. prior text for NER
/// sliding-window).
#[derive(Debug, Clone, Deref, DerefMut, From)]
pub struct SequentialContext<T = ()> {
    /// The data carried by this context.
    pub data: T,
}

impl<T: Send + Sync + 'static> private::Sealed for SequentialContext<T> {}
impl<T: Send + Sync + 'static> OperationContext for SequentialContext<T> {}

impl Default for SequentialContext<()> {
    fn default() -> Self {
        Self { data: () }
    }
}

impl<T> SequentialContext<T> {
    /// Create a new sequential context with the given data.
    pub fn new(data: T) -> Self {
        Self { data }
    }

    /// Transform the inner data, preserving the context wrapper.
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> SequentialContext<U> {
        SequentialContext { data: f(self.data) }
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
