//! HTML handler: holds parsed HTML content and provides location-based
//! access via [`Handler`] + [`Handle`].
//!
//! [`Handle::locations`] yields one location per text node in
//! document order; offsets are cumulative over the text-node sequence
//! (not raw HTML bytes). [`Handler::encode`] reconstructs the HTML by
//! re-parsing the original source into a DOM, applying mutations, and
//! serializing back with [`Html::html`].
//!
//! [`Html::html`]: scraper::Html::html

use nvisy_codec::core::{Handle, Located, LocationStream};
use nvisy_codec::handler::{Handler, TextData, TextRedaction};
use nvisy_core::Error;
use nvisy_core::content::{ContentData, ContentSource, DocumentType};
use nvisy_core::modality::Text;

use super::redact;

const TARGET: &str = "html-handler";

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
impl Handle<Text> for HtmlHandler {
    fn locations(&self) -> LocationStream<'_, Text> {
        let source = self.source;
        let mut items = Vec::with_capacity(self.data.text_nodes.len());
        let mut offset = 0usize;
        for text in &self.data.text_nodes {
            let start = offset;
            let end = start + text.len();
            items.push(Located::new(
                source,
                Text {
                    start,
                    end,
                    ..Default::default()
                },
            ));
            offset = end;
        }
        LocationStream::new(futures::stream::iter(items))
    }

    async fn read(&self, location: &Text) -> Option<TextData> {
        let offsets = self.node_offsets();
        let idx = offsets
            .iter()
            .position(|&(start, _)| start == location.start)?;
        self.data.text_nodes.get(idx).cloned().map(TextData::from)
    }

    async fn redact_at(&mut self, location: &Text, redaction: TextRedaction) -> Result<(), Error> {
        let offsets = self.node_offsets();
        let Some(idx) = offsets
            .iter()
            .position(|&(start, end)| location.start >= start && location.end <= end)
        else {
            return Ok(());
        };
        let node_start = offsets[idx].0;
        let start = location.start - node_start;
        let end = location.end - node_start;
        let value = redaction.output().replacement_value().unwrap_or_default();
        redact::replace_range(&mut self.data.text_nodes[idx], value, start, end, TARGET)
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
    use futures::StreamExt;
    use nvisy_codec::core::{Handle, Redactions};
    use nvisy_codec::handler::TextOutput;
    use nvisy_core::Error;

    use super::*;

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
    async fn encode_after_redact() -> Result<(), Error> {
        let raw = "<html><head></head><body><p>Hello</p><p>World</p></body></html>";
        let mut h = handler_from_html(raw);
        let items: Vec<_> = h.locations().collect().await;
        let mut rs = Redactions::new();
        rs.push(
            items[0].location.clone(),
            TextRedaction::new(TextOutput::replace("[REDACTED]")),
        );
        h.redact(rs).await?;
        let result = h.encode()?.as_str().unwrap().to_owned();
        assert!(result.contains("[REDACTED]"));
        assert!(result.contains("World"));
        Ok(())
    }

    #[tokio::test]
    async fn locations_returns_text_nodes() {
        let h = handler_from_html("<html><head></head><body><p>Alpha</p><p>Beta</p></body></html>");
        let items: Vec<_> = h.locations().collect().await;
        assert_eq!(items.len(), 2);
    }
}
