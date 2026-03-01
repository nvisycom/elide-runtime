//! Async span stream for [`Handler::view_spans`].
//!
//! [`Handler::view_spans`]: crate::handler::Handler::view_spans

use std::pin::Pin;
use std::task::{Context, Poll};

use futures::Stream;

use crate::handler::Span;

/// Async stream of spans returned by [`Handler::view_spans`].
///
/// Wraps a `Pin<Box<dyn Stream>>` so that handler implementations
/// can return any iterator/stream without exposing a concrete type.
///
/// [`Handler::view_spans`]: crate::handler::Handler::view_spans
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
