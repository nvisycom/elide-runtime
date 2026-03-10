//! Async span stream for viewing and editing handler content.

use std::pin::Pin;
use std::task::{Context, Poll};

use futures::Stream;

use super::Span;

/// Async stream of spans returned by capability trait methods.
///
/// Wraps a `Pin<Box<dyn Stream>>` so that handler implementations
/// can return any iterator/stream without exposing a concrete type.
///
/// Used both for reading spans from a handler and for sending edits
/// back.
pub struct SpanStream<'a, Id, Data> {
    inner: Pin<Box<dyn Stream<Item = Span<Id, Data>> + Send + 'a>>,
}

impl<'a, Id, Data> SpanStream<'a, Id, Data> {
    /// Wrap any `Send` stream of spans.
    pub fn new(stream: impl Stream<Item = Span<Id, Data>> + Send + 'a) -> Self {
        Self {
            inner: Box::pin(stream),
        }
    }
}

impl<Id, Data> Unpin for SpanStream<'_, Id, Data> {}

impl<Id, Data> Stream for SpanStream<'_, Id, Data> {
    type Item = Span<Id, Data>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.inner.as_mut().poll_next(cx)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}
