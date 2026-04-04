//! HTML handler: holds parsed HTML content and provides span-based
//! access via [`Handler`] + [`TextHandler`].
//!
//! The handler stores extracted text nodes so the content can be
//! inspected and edited without holding the full DOM.
//!
//! # Span model
//!
//! [`TextHandler::text_spans`] yields one [`Span`] per text node in
//! document order, addressed by [`TextLocation`] with byte offsets
//! computed from cumulative text node lengths.
//!
//! # Encoding
//!
//! [`Handler::encode`] reconstructs the HTML by re-parsing the
//! original source into a DOM, applying edits via direct node
//! mutation, and serializing back with [`scraper::Html::html`].

use futures::StreamExt;
use nvisy_core::Error;
use nvisy_core::content::{ContentData, ContentSource};
use nvisy_core::media::DocumentType;
use nvisy_ontology::entity::TextLocation;

use crate::document::{Span, SpanStream};
use crate::handler::text::TextData;
use crate::handler::{Handler, TextHandler};

/// Parsed HTML content stored as extracted text nodes.
#[derive(Debug, Clone)]
pub struct HtmlData {
    /// Text nodes extracted in document order.
    pub text_nodes: Vec<String>,
    /// The raw HTML source (kept for reconstruction).
    pub raw: String,
}

/// Handler for loaded HTML content.
#[derive(Debug)]
pub struct HtmlHandler {
    source: ContentSource,
    data: HtmlData,
}

impl Handler for HtmlHandler {
    fn document_type(&self) -> DocumentType {
        DocumentType::Html
    }

    fn source(&self) -> ContentSource {
        self.source
    }

    #[tracing::instrument(name = "html.encode", skip_all, fields(output_bytes))]
    fn encode(&self) -> Result<ContentData, Error> {
        let mut dom = scraper::Html::parse_document(&self.data.raw);

        let text_node_ids: Vec<_> = dom
            .tree
            .nodes()
            .filter(|node| node.value().is_text())
            .map(|node| node.id())
            .collect();

        for (i, &node_id) in text_node_ids.iter().enumerate() {
            let current: &str = &self.data.text_nodes[i];
            if let Some(mut node_mut) = dom.tree.get_mut(node_id)
                && let scraper::node::Node::Text(t) = node_mut.value()
                && t.text.as_ref() != current
            {
                t.text = current.into();
            }
        }

        let bytes = dom.html().into_bytes();
        tracing::Span::current().record("output_bytes", bytes.len());
        let source = ContentSource::new().with_parent(&self.source);
        Ok(ContentData::new(source, bytes.into()))
    }
}

#[async_trait::async_trait]
impl TextHandler for HtmlHandler {
    async fn text_spans(&self) -> SpanStream<'_, TextLocation, TextData> {
        let mut spans = Vec::with_capacity(self.data.text_nodes.len());
        let mut offset = 0usize;

        for text in &self.data.text_nodes {
            let start = offset;
            let end = start + text.len();
            spans.push(
                Span::new(
                    TextLocation {
                        start_offset: start,
                        end_offset: end,
                        ..Default::default()
                    },
                    TextData::from(text.clone()),
                )
                .with_source(self.source),
            );
            offset = end;
        }

        SpanStream::new(futures::stream::iter(spans))
    }

    async fn edit_text(
        &mut self,
        edits: SpanStream<'_, TextLocation, TextData>,
    ) -> Result<(), Error> {
        let edits: Vec<_> = edits.collect().await;
        let offsets = self.node_offsets();
        for edit in edits {
            let idx = offsets
                .iter()
                .position(|&(start, _)| start == edit.id.start_offset)
                .ok_or_else(|| {
                    Error::validation(
                        format!("no text node at byte offset {}", edit.id.start_offset),
                        "html-handler",
                    )
                })?;
            self.data.text_nodes[idx] = edit.data.into_inner();
        }
        Ok(())
    }

    async fn value_at(&self, location: &TextLocation) -> Option<String> {
        let offsets = self.node_offsets();
        let idx = offsets
            .iter()
            .position(|&(start, _)| start == location.start_offset)?;
        self.data.text_nodes.get(idx).cloned()
    }
}

impl HtmlHandler {
    /// Create a new handler from parsed HTML data.
    pub fn new(data: HtmlData) -> Self {
        Self {
            source: ContentSource::new(),
            data,
        }
    }

    /// Set the content source for lineage tracking.
    pub fn with_source(mut self, source: ContentSource) -> Self {
        self.source = source;
        self
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

    fn node_offsets(&self) -> Vec<(usize, usize)> {
        let mut offset = 0;
        self.data
            .text_nodes
            .iter()
            .map(|text| {
                let start = offset;
                let end = start + text.len();
                offset = end;
                (start, end)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use nvisy_core::Error;

    use super::*;
    use crate::document::Span;
    use crate::handler::{Handler, TextHandler};

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
        let content = h.encode()?;
        assert_eq!(content.as_str().unwrap(), raw);
        Ok(())
    }

    #[tokio::test]
    async fn encode_after_edit() -> Result<(), Error> {
        let raw = "<html><head></head><body><p>Hello</p><p>World</p></body></html>";
        let mut h = handler_from_html(raw);
        let spans: Vec<_> = h.text_spans().await.collect().await;
        h.edit_text(SpanStream::new(futures::stream::iter(vec![Span::new(
            spans[0].id.clone(),
            "[REDACTED]".into(),
        )])))
        .await?;
        let result = h.encode()?.as_str().unwrap().to_owned();
        assert!(result.contains("[REDACTED]"));
        assert!(result.contains("World"));
        Ok(())
    }

    #[tokio::test]
    async fn view_spans_returns_text() {
        let h =
            handler_from_html("<html><head></head><body><p>Alpha</p><p>Beta</p></body></html>");
        let spans: Vec<_> = h.text_spans().await.collect().await;
        assert_eq!(spans.len(), 2);
        assert_eq!(spans[0].data, "Alpha");
        assert_eq!(spans[1].data, "Beta");
    }

    #[tokio::test]
    async fn value_at_returns_text_node() {
        let h = handler_from_html("<html><head></head><body><p>Hello</p></body></html>");
        let spans: Vec<_> = h.text_spans().await.collect().await;
        assert_eq!(h.value_at(&spans[0].id).await, Some("Hello".to_string()));
    }
}
