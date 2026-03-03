//! Async span edit stream for editing handler content.

use std::pin::Pin;
use std::task::{Context, Poll};

use futures::Stream;

use super::SpanEdit;

/// Async stream of edits consumed by capability trait edit methods.
///
/// Wraps a `Pin<Box<dyn Stream>>` so that callers can pass any
/// iterator/stream of edits without exposing a concrete type.
pub struct SpanEditStream<'a, Id, Data> {
    inner: Pin<Box<dyn Stream<Item = SpanEdit<Id, Data>> + Send + 'a>>,
}

impl<'a, Id, Data> SpanEditStream<'a, Id, Data> {
    /// Wrap any `Send` stream of span edits.
    pub fn new(stream: impl Stream<Item = SpanEdit<Id, Data>> + Send + 'a) -> Self {
        Self {
            inner: Box::pin(stream),
        }
    }
}

impl<Id, Data> Unpin for SpanEditStream<'_, Id, Data> {}

impl<Id, Data> Stream for SpanEditStream<'_, Id, Data> {
    type Item = SpanEdit<Id, Data>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.inner.as_mut().poll_next(cx)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}
