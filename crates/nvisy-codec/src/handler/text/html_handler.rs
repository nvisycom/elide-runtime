//! HTML handler: holds parsed HTML content and streams its text
//! nodes via [`Handle<Text>`], with random-access reads / redactions
//! via [`Handle<Text>`].
//!
//! Offsets are cumulative over the **text-node sequence** in document
//! order, not raw HTML bytes. [`Handler::encode`] reconstructs the
//! HTML by re-parsing the original source into a DOM, applying
//! mutations, and serializing back with [`Html::html`].
//!
//! [`Html::html`]: scraper::Html::html

use std::ops::Range;
use std::sync::Arc;

use nvisy_core::Error;
use nvisy_core::modality::{Text, TextData, TextLocation};
use nvisy_core::redaction::{Redactions, TextReplacement};

use super::{HtmlLoader, lift_identity, redact};
use crate::content::{ContentData, ContentSource};
use crate::core::{Chunk, Handle, Handler, ModalityKind};
use crate::{Format, FormatId, LoaderAdapter};

const TARGET: &str = "html-handler";

/// Stable [`FormatId`] for the HTML codec.
pub const FORMAT_ID: FormatId = FormatId::from_static("nvisy.text.html");

/// [`Format`] descriptor registered into [`crate::CodecRegistry`].
pub fn format() -> Format {
    Format {
        id: FORMAT_ID.clone(),
        modality: ModalityKind::Text,
        extensions: vec!["html".into(), "htm".into()],
        content_types: vec!["text/html".into()],
        loader: Arc::new(LoaderAdapter::new(HtmlLoader::default())),
    }
}

/// Parsed HTML content: extracted text nodes alongside the raw source
/// (kept for round-trip reconstruction).
#[derive(Debug, Clone)]
pub struct HtmlData {
    /// Text nodes extracted in document order.
    pub text_nodes: Vec<String>,
    /// The raw HTML source.
    pub raw: String,
}

/// Handler for loaded HTML content.
///
/// `node_starts` is a cumulative-offset index over the text-node
/// sequence: `node_starts[i]` is the byte position of text node `i`,
/// and `node_starts[text_nodes.len()]` is the total length sentinel.
/// Maintained on every redaction so random-access reads are
/// `O(log N)` instead of rebuilding the table per call.
#[derive(Debug)]
pub struct HtmlHandler {
    source: ContentSource,
    data: HtmlData,
    node_starts: Vec<usize>,
    cursor: usize,
}

impl Handler for HtmlHandler {
    fn format(&self) -> FormatId {
        FORMAT_ID.clone()
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
    async fn next_chunk(&mut self) -> Result<Option<Chunk<Text>>, Error> {
        if self.cursor >= self.data.text_nodes.len() {
            return Ok(None);
        }
        let i = self.cursor;
        let start = self.node_starts[i];
        let end = self.node_starts[i + 1];
        let text = &self.data.text_nodes[i];
        self.cursor += 1;
        Ok(Some(Chunk {
            location: TextLocation {
                start,
                end,
                ..Default::default()
            },
            data: TextData::from(text.as_str()),
            embed: None,
        }))
    }

    fn lift_chunk(&self, chunk: &Chunk<Text>, value_range: Range<usize>) -> Option<TextLocation> {
        lift_identity(chunk, value_range)
    }

    async fn read(&self, location: &TextLocation) -> Result<Option<TextData>, Error> {
        let Some(i) = self.node_for(location.start) else {
            return Ok(None);
        };
        let node_start = self.node_starts[i];
        let node_end = self.node_starts[i + 1];
        if location.end > node_end {
            return Ok(None);
        }
        let local_start = location.start - node_start;
        let local_end = location.end - node_start;
        Ok(self.data.text_nodes[i]
            .get(local_start..local_end)
            .map(TextData::from))
    }

    async fn redact(&mut self, redactions: Redactions<Text>) -> Result<(), Error> {
        let mut items = redactions.into_items();
        items.sort_by_key(|(loc, _)| std::cmp::Reverse(loc.start));
        for (location, replacement) in items {
            self.redact_one(&location, replacement)?;
        }
        Ok(())
    }
}

impl HtmlHandler {
    /// Create a new handler from parsed HTML data.
    pub fn new(data: HtmlData) -> Self {
        let node_starts = compute_node_starts(&data.text_nodes);
        Self {
            source: ContentSource::new(),
            data,
            node_starts,
            cursor: 0,
        }
    }

    /// Attach a content source for lineage tracking.
    pub fn with_source(mut self, source: ContentSource) -> Self {
        self.source = source;
        self
    }

    /// All extracted text nodes in document order.
    pub fn text_nodes(&self) -> &[String] {
        &self.data.text_nodes
    }

    /// A specific text node by 0-based index.
    pub fn text_node(&self, index: usize) -> Option<&str> {
        self.data.text_nodes.get(index).map(String::as_str)
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

    /// Rewind the streaming cursor to the start of the document.
    pub fn rewind(&mut self) {
        self.cursor = 0;
    }

    fn node_for(&self, byte_offset: usize) -> Option<usize> {
        match self.node_starts.binary_search(&byte_offset) {
            Ok(i) if i < self.data.text_nodes.len() => Some(i),
            Ok(_) => None,
            Err(i) if i > 0 && i <= self.data.text_nodes.len() => Some(i - 1),
            _ => None,
        }
    }

    fn shift_starts_after(&mut self, i: usize, delta: isize) {
        if delta == 0 {
            return;
        }
        for s in &mut self.node_starts[i + 1..] {
            *s = (*s as isize + delta) as usize;
        }
    }

    fn redact_one(
        &mut self,
        location: &TextLocation,
        replacement: TextReplacement,
    ) -> Result<(), Error> {
        let Some(i) = self.node_for(location.start) else {
            return Ok(());
        };
        let node_start = self.node_starts[i];
        let node_end = self.node_starts[i + 1];
        if location.end > node_end {
            return Ok(());
        }
        let local_start = location.start - node_start;
        let local_end = location.end - node_start;
        let value = replacement.replacement_value().unwrap_or_default();
        let before_len = self.data.text_nodes[i].len();
        redact::replace_range(
            &mut self.data.text_nodes[i],
            value,
            local_start,
            local_end,
            TARGET,
        )?;
        let delta = self.data.text_nodes[i].len() as isize - before_len as isize;
        self.shift_starts_after(i, delta);
        Ok(())
    }
}

fn compute_node_starts(text_nodes: &[String]) -> Vec<usize> {
    let mut starts = Vec::with_capacity(text_nodes.len() + 1);
    let mut offset = 0usize;
    for text in text_nodes {
        starts.push(offset);
        offset += text.len();
    }
    starts.push(offset);
    starts
}

#[cfg(test)]
mod tests {
    use nvisy_core::Error;
    use nvisy_core::redaction::TextReplacement;

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
        let first = h.next_chunk().await?.unwrap();
        let loc = first.location.clone();
        let mut rs = Redactions::new();
        rs.push(loc, TextReplacement::substituted("[REDACTED]"));
        h.redact(rs).await?;
        let result = h.encode()?.as_str().unwrap().to_owned();
        assert!(result.contains("[REDACTED]"));
        assert!(result.contains("World"));
        Ok(())
    }

    #[tokio::test]
    async fn stream_yields_each_text_node() -> Result<(), Error> {
        let mut h =
            handler_from_html("<html><head></head><body><p>Alpha</p><p>Beta</p></body></html>");
        let mut count = 0;
        while h.next_chunk().await?.is_some() {
            count += 1;
        }
        assert_eq!(count, 2);
        Ok(())
    }
}
