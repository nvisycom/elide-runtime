//! HTML handler: holds parsed HTML content and provides span-based
//! access via [`Handler`].
//!
//! The handler stores extracted text nodes so the content can be
//! inspected and edited without holding the full DOM.
//!
//! # Span model
//!
//! [`Handler::view_spans`] yields one [`Span`] per text node in
//! document order.  Each span is addressed by a [`HtmlSpan`]
//! (0-based text-node index) and carries the text content as a
//! `String`.
//!
//! [`Handler::edit_spans`] replaces the content of text nodes at the
//! given indices.
//!
//! # Encoding
//!
//! [`Handler::encode`] reconstructs the HTML by re-parsing the
//! original source into a DOM, applying edits via direct node
//! mutation, and serializing back with [`scraper::Html::html`].

use futures::StreamExt;

use nvisy_core::Error;
use nvisy_core::fs::DocumentType;

use crate::stream::{SpanEditStream, SpanStream};
use crate::handler::{Handler, Span};
use crate::handler::text::TextData;
use crate::transform::TextHandler;

/// 0-based index of a text node within the HTML document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HtmlSpan(pub usize);

/// Parsed HTML content stored as extracted text nodes.
#[derive(Debug, Clone)]
pub struct HtmlData {
    /// Text nodes extracted in document order.
    pub text_nodes: Vec<String>,
    /// The raw HTML source (kept for reconstruction).
    pub raw: String,
}

/// Handler for loaded HTML content.
///
/// Each text node is independently addressable via [`HtmlSpan`].
#[derive(Debug, Clone)]
pub struct HtmlHandler {
    pub(crate) data: HtmlData,
}

#[async_trait::async_trait]
impl Handler for HtmlHandler {
    fn document_type(&self) -> DocumentType {
        DocumentType::Html
    }

    #[tracing::instrument(name = "html.encode", skip_all, fields(output_bytes))]
    fn encode(&self) -> Result<bytes::Bytes, Error> {
        // Re-parse the original source into a mutable DOM.
        let mut dom = scraper::Html::parse_document(&self.data.raw);

        // Collect text-node IDs in document order.
        let text_node_ids: Vec<_> = dom
            .tree
            .nodes()
            .filter(|node| node.value().is_text())
            .map(|node| node.id())
            .collect();

        // Mutate changed text nodes directly in the DOM.
        for (i, &node_id) in text_node_ids.iter().enumerate() {
            let current: &str = &self.data.text_nodes[i];
            if let Some(mut node_mut) = dom.tree.get_mut(node_id)
                && let scraper::node::Node::Text(t) = node_mut.value()
                && t.text.as_ref() != current
            {
                t.text = current.into();
            }
        }

        // Serialize the mutated DOM back to HTML.
        let bytes = dom.html().into_bytes();
        tracing::Span::current().record("output_bytes", bytes.len());
        Ok(bytes.into())
    }

    type SpanId = HtmlSpan;
    type SpanData = TextData;

    async fn view_spans(&self) -> SpanStream<'_, HtmlSpan, TextData> {
        SpanStream::new(futures::stream::iter(HtmlSpanIter {
            nodes: &self.data.text_nodes,
            index: 0,
        }))
    }

    async fn edit_spans(
        &mut self,
        edits: SpanEditStream<'_, HtmlSpan, TextData>,
    ) -> Result<(), Error> {
        let edits: Vec<_> = edits.collect().await;
        for edit in edits {
            let node = self.data.text_nodes.get_mut(edit.id.0).ok_or_else(|| {
                Error::validation(
                    format!("text node index out of bounds: {}", edit.id.0),
                    "html-handler",
                )
            })?;
            *node = edit.data.into_inner();
        }
        Ok(())
    }
}

impl HtmlHandler {
    /// Create a new handler from parsed HTML data.
    pub fn new(data: HtmlData) -> Self {
        Self { data }
    }

    /// All extracted text nodes.
    pub fn text_nodes(&self) -> &[String] {
        &self.data.text_nodes
    }

    /// A specific text node by 0-based index.
    pub fn text_node(&self, index: usize) -> Option<&str> {
        self.data.text_nodes.get(index).map(|s| s.as_str())
    }

    /// Total number of text nodes.
    pub fn len(&self) -> usize {
        self.data.text_nodes.len()
    }

    /// Whether the document has no text nodes.
    pub fn is_empty(&self) -> bool {
        self.data.text_nodes.is_empty()
    }

    /// The raw HTML source.
    pub fn raw(&self) -> &str {
        &self.data.raw
    }

    /// Consume the handler and return the inner [`HtmlData`].
    pub fn into_data(self) -> HtmlData {
        self.data
    }
}

impl TextHandler for HtmlHandler {}

/// Iterator over text nodes of an HTML document.
struct HtmlSpanIter<'a> {
    nodes: &'a [String],
    index: usize,
}

impl<'a> Iterator for HtmlSpanIter<'a> {
    type Item = Span<HtmlSpan, TextData>;

    fn next(&mut self) -> Option<Self::Item> {
        let text = self.nodes.get(self.index)?;
        let span = Span::new(HtmlSpan(self.index), TextData::from(text.clone()));
        self.index += 1;
        Some(span)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.nodes.len() - self.index;
        (remaining, Some(remaining))
    }
}

impl ExactSizeIterator for HtmlSpanIter<'_> {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handler::{Handler, SpanEdit};
    use nvisy_core::Error;

    fn handler_from_html(raw: &str) -> HtmlHandler {
        let dom = scraper::Html::parse_document(raw);
        let text_nodes: Vec<String> = dom
            .tree
            .nodes()
            .filter_map(|node| {
                if let scraper::node::Node::Text(t) = node.value() {
                    Some(t.text.to_string())
                } else {
                    None
                }
            })
            .collect();
        HtmlHandler::new(HtmlData {
            text_nodes,
            raw: raw.to_string(),
        })
    }

    #[test]
    fn encode_unchanged() -> Result<(), Error> {
        let raw = "<html><head></head><body><p>Hello</p></body></html>";
        let h = handler_from_html(raw);
        let bytes = h.encode()?;
        assert_eq!(std::str::from_utf8(&bytes).unwrap(), raw);
        Ok(())
    }

    #[tokio::test]
    async fn encode_after_edit_spans() -> Result<(), Error> {
        let raw = "<html><head></head><body><p>Hello</p><p>World</p></body></html>";
        let mut h = handler_from_html(raw);
        h.edit_spans(SpanEditStream::new(futures::stream::iter(vec![
            SpanEdit::new(HtmlSpan(0), "[REDACTED]".into()),
        ])))
        .await?;
        let result = std::str::from_utf8(&h.encode()?).unwrap().to_owned();
        assert!(result.contains("[REDACTED]"));
        assert!(result.contains("World"));
        assert!(result.contains("<p>"));
        Ok(())
    }

    #[test]
    fn encode_preserves_tags() -> Result<(), Error> {
        let h = handler_from_html("<html><head></head><body><div><span>foo</span> bar</div></body></html>");
        let mut h = h;
        h.data.text_nodes[0] = "baz".to_string();
        let result = std::str::from_utf8(&h.encode()?).unwrap().to_owned();
        assert!(result.contains("<span>baz</span>"));
        assert!(result.contains(" bar"));
        Ok(())
    }

    #[tokio::test]
    async fn encode_duplicate_text_nodes() -> Result<(), Error> {
        let raw = "<html><head></head><body><p>hello</p><p>hello</p></body></html>";
        let mut h = handler_from_html(raw);
        // Edit only the first "hello" — the second should remain unchanged.
        h.edit_spans(SpanEditStream::new(futures::stream::iter(vec![
            SpanEdit::new(HtmlSpan(0), "FIRST".into()),
        ])))
        .await?;
        let result = std::str::from_utf8(&h.encode()?).unwrap().to_owned();
        assert!(result.contains("<p>FIRST</p>"));
        assert!(result.contains("<p>hello</p>"));
        Ok(())
    }

    #[tokio::test]
    async fn view_spans_returns_text() {
        let h = handler_from_html("<html><head></head><body><p>Alpha</p><p>Beta</p></body></html>");
        let spans: Vec<_> = h.view_spans().await.collect().await;
        assert_eq!(spans.len(), 2);
        assert_eq!(spans[0].data, "Alpha");
        assert_eq!(spans[0].id, HtmlSpan(0));
        assert_eq!(spans[1].data, "Beta");
        assert_eq!(spans[1].id, HtmlSpan(1));
    }

    #[tokio::test]
    async fn edit_spans_out_of_bounds() {
        let mut h = handler_from_html("<html><head></head><body><p>only</p></body></html>");
        let err = h
            .edit_spans(SpanEditStream::new(futures::stream::iter(vec![
                SpanEdit::new(HtmlSpan(99), "nope".into()),
            ])))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("out of bounds"));
    }
}
