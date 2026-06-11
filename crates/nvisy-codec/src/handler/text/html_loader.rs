//! HTML loader: validates and parses raw HTML content into an
//! [`HtmlHandler`].
//!
//! Parses the input using [`scraper`], extracts text nodes in document
//! order, and produces a handler backed by those nodes plus the raw
//! source (used to reconstruct the HTML after edits).

use nvisy_core::Error;
use nvisy_core::modality::Text;
use scraper::Html;

use super::{HtmlData, HtmlHandler};
use crate::Loader;
use crate::content::{ContentData, ContentSource, TextEncoding};

/// Loader for HTML files. Produces one [`HtmlHandler`] per input.
#[derive(Debug, Default)]
pub struct HtmlLoader {
    /// Character encoding of the input bytes. Defaults to UTF-8.
    pub encoding: TextEncoding,
}

#[async_trait::async_trait]
impl Loader<Text> for HtmlLoader {
    type Handler = HtmlHandler;

    #[tracing::instrument(name = "html.decode", skip_all, fields(input_bytes, text_nodes))]
    async fn decode(&self, content: ContentData) -> Result<HtmlHandler, Error> {
        let parent = content.content_source;
        let raw = content.to_bytes();
        tracing::Span::current().record("input_bytes", raw.len());
        let text = self.encoding.decode_bytes(&raw, "html-loader")?;
        let dom = Html::parse_document(&text);

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
        tracing::Span::current().record("text_nodes", text_nodes.len());

        let source = ContentSource::new().with_parent(&parent);
        Ok(HtmlHandler::new(HtmlData {
            text_nodes,
            raw: text,
        })
        .with_source(source))
    }
}
