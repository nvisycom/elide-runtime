//! Text rendering and redaction primitives.
//!
//! Provides the [`TextHandler`] async trait that text-bearing handlers
//! implement to support span-aware redaction.  The default implementation
//! groups redactions by [`SpanId`](Handler::SpanId), reads current content
//! via [`Handler::view_spans`], applies intra-span byte-offset replacements
//! right-to-left, and writes the results back via [`Handler::edit_spans`].

mod mask;
mod output;

pub use output::TextRedactionOutput;

use std::collections::HashMap;
use std::hash::Hash;

use futures::StreamExt;

use crate::document::edit_stream::SpanEditStream;
use crate::handler::{Handler, SpanEdit};
use nvisy_core::error::Error;

/// A located text redaction: pairs a span identifier and intra-span byte
/// range with a [`TextRedactionOutput`] that carries the replacement.
pub struct TextRedaction<S> {
    /// Which span this redaction targets.
    pub span_id: S,
    /// Byte offset where the redacted region starts within the span.
    pub start: usize,
    /// Byte offset where the redacted region ends (exclusive) within the span.
    pub end: usize,
    /// The redaction output that carries the replacement value.
    pub output: TextRedactionOutput,
}

/// Trait for handlers that support text redaction.
///
/// Extends [`Handler`] with [`redact_spans`](Self::redact_spans) which
/// applies a batch of span-aware text redactions.  The provided default
/// implementation groups redactions by span, reads content via
/// [`view_spans`](Handler::view_spans), applies byte-offset replacements
/// right-to-left per span, and writes back via
/// [`edit_spans`](Handler::edit_spans).
#[async_trait::async_trait]
pub trait TextHandler: Handler
where
    Self::SpanId: Eq + Hash,
    Self::SpanData: AsRef<str> + From<String>,
{
    /// Apply a batch of text redactions, mutating in place.
    ///
    /// Each [`TextRedaction`] identifies a span and an intra-span byte
    /// range together with a [`TextRedactionOutput`] whose replacement
    /// value is written into the content.  Replacements within each span
    /// are applied right-to-left so that byte offsets remain valid.
    async fn redact_spans(
        &mut self,
        redactions: &[TextRedaction<Self::SpanId>],
    ) -> Result<(), Error> {
        if redactions.is_empty() {
            return Ok(());
        }

        // Group redactions by span id.
        let mut by_span: HashMap<&Self::SpanId, Vec<(usize, usize, String)>> = HashMap::new();
        for r in redactions {
            let value = r
                .output
                .replacement_value()
                .unwrap_or_default()
                .to_string();
            by_span
                .entry(&r.span_id)
                .or_default()
                .push((r.start, r.end, value));
        }

        // Read current content for affected spans.
        let all_spans: Vec<_> = self.view_spans().await.collect().await;

        let mut edits: Vec<SpanEdit<Self::SpanId, Self::SpanData>> = Vec::new();
        for span in &all_spans {
            if let Some(replacements) = by_span.get_mut(&span.id) {
                let content = span.data.as_ref();

                // Sort right-to-left so earlier byte offsets stay valid.
                replacements.sort_by(|a, b| b.0.cmp(&a.0));

                let mut result = content.to_string();
                for (start, end, value) in replacements.iter() {
                    let s = (*start).min(result.len());
                    let e = (*end).min(result.len());
                    if s >= e {
                        continue;
                    }
                    result = format!("{}{}{}", &result[..s], value, &result[e..]);
                }

                edits.push(SpanEdit {
                    id: span.id.clone(),
                    data: Self::SpanData::from(result),
                });
            }
        }

        if !edits.is_empty() {
            self.edit_spans(SpanEditStream::new(futures::stream::iter(edits)))
                .await?;
        }

        Ok(())
    }
}
