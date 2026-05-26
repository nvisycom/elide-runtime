//! Async location stream returned by handler `locations()` methods.

use std::pin::Pin;
use std::task::{Context, Poll};

use futures::Stream;

use super::Located;

/// Async stream of [`Located<L>`] items returned by handler
/// capability traits.
///
/// Wraps a `Pin<Box<dyn Stream>>` so handlers can return any
/// iterator/stream without exposing a concrete type.
pub struct LocationStream<'a, L> {
    inner: Pin<Box<dyn Stream<Item = Located<L>> + Send + 'a>>,
}

impl<'a, L> LocationStream<'a, L> {
    /// Wrap any `Send` stream of located locations.
    pub fn new(stream: impl Stream<Item = Located<L>> + Send + 'a) -> Self {
        Self {
            inner: Box::pin(stream),
        }
    }

    /// Construct an empty stream.
    pub fn empty() -> Self
    where
        L: Send + 'a,
    {
        Self::new(futures::stream::empty())
    }
}

impl<L> Unpin for LocationStream<'_, L> {}

impl<L> Stream for LocationStream<'_, L> {
    type Item = Located<L>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.inner.as_mut().poll_next(cx)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}
