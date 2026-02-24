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

use futures::StreamExt;

use nvisy_core::Error;
use nvisy_core::fs::DocumentType;

use crate::document::{SpanEditStream, SpanStream};
use crate::handler::{Handler, Span};
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
    fn encode(&self) -> Result<Vec<u8>, Error> {
        let mut result = self.data.raw.clone();
        let dom = scraper::Html::parse_document(&self.data.raw);

        // Collect original text nodes in document order
        let original_nodes: Vec<&str> = dom
            .tree
            .nodes()
            .filter_map(|node| {
                if let scraper::node::Node::Text(t) = node.value() {
                    Some(t.text.as_ref())
                } else {
                    None
                }
            })
            .collect();

        // Build patches for changed nodes, then apply right-to-left
        let mut patches: Vec<(usize, usize, &str)> = Vec::new();
        let mut search_start = 0;
        for (i, original) in original_nodes.iter().enumerate() {
            let Some(pos) = result[search_start..].find(original) else {
                continue;
            };
            let abs_pos = search_start + pos;
            if i < self.data.text_nodes.len() && *original != self.data.text_nodes[i] {
                patches.push((abs_pos, abs_pos + original.len(), &self.data.text_nodes[i]));
            }
            search_start = abs_pos + original.len();
        }

        // Apply patches right-to-left to preserve positions
        for (start, end, replacement) in patches.into_iter().rev() {
            result.replace_range(start..end, replacement);
        }

        let bytes = result.into_bytes();
        tracing::Span::current().record("output_bytes", bytes.len());
        Ok(bytes)
    }

    type SpanId = HtmlSpan;
    type SpanData = String;

    async fn view_spans(&self) -> SpanStream<'_, HtmlSpan, String> {
        SpanStream::new(futures::stream::iter(HtmlSpanIter {
            nodes: &self.data.text_nodes,
            index: 0,
        }))
    }

    async fn edit_spans(
        &mut self,
        edits: SpanEditStream<'_, HtmlSpan, String>,
    ) -> Result<(), Error> {
        let edits: Vec<_> = edits.collect().await;
        for edit in edits {
            let node = self.data.text_nodes.get_mut(edit.id.0).ok_or_else(|| {
                Error::validation(
                    format!("text node index out of bounds: {}", edit.id.0),
                    "html-handler",
                )
            })?;
            *node = edit.data;
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
    type Item = Span<HtmlSpan, String>;

    fn next(&mut self) -> Option<Self::Item> {
        let text = self.nodes.get(self.index)?;
        let span = Span::new(HtmlSpan(self.index), text.clone());
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
    use crate::handler::Handler;
    use nvisy_core::Error;

    #[test]
    fn encode_unchanged() -> Result<(), Error> {
        let raw = "<p>Hello</p>".to_string();
        let h = HtmlHandler::new(HtmlData {
            text_nodes: vec!["Hello".to_string()],
            raw: raw.clone(),
        });
        let bytes = h.encode()?;
        assert_eq!(String::from_utf8(bytes).expect("valid utf-8"), raw);
        Ok(())
    }

    #[test]
    fn encode_after_edit() -> Result<(), Error> {
        let raw = "<p>Hello</p><p>World</p>".to_string();
        let mut h = HtmlHandler::new(HtmlData {
            text_nodes: vec!["Hello".to_string(), "World".to_string()],
            raw,
        });
        // Edit the first text node
        h.data.text_nodes[0] = "[REDACTED]".to_string();
        let bytes = h.encode()?;
        let result = String::from_utf8(bytes).expect("valid utf-8");
        assert!(result.contains("[REDACTED]"));
        assert!(result.contains("<p>"));
        assert!(result.contains("World"));
        Ok(())
    }

    #[test]
    fn encode_preserves_tags() -> Result<(), Error> {
        let raw = "<div><span>foo</span> bar</div>".to_string();
        let mut h = HtmlHandler::new(HtmlData {
            text_nodes: vec!["foo".to_string(), " bar".to_string()],
            raw,
        });
        h.data.text_nodes[0] = "baz".to_string();
        let result = String::from_utf8(h.encode()?).expect("valid utf-8");
        assert_eq!(result, "<div><span>baz</span> bar</div>");
        Ok(())
    }
}
