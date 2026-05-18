//! HTML loader: validates and parses raw HTML content into a
//! [`HtmlHandler`].
//!
//! The loader parses the input using [`scraper`], extracts text nodes
//! in document order, and produces a handler backed by those nodes.
//!
//! [`scraper`]: https://docs.rs/scraper

use nvisy_core::Error;
use nvisy_core::content::{ContentData, ContentSource};
use scraper::Html;

use crate::handler::{HtmlData, HtmlHandler, Loader};

/// Parameters for [`HtmlLoader`].
#[derive(Debug, Default)]
pub struct HtmlParams {
    /// Character encoding of the input bytes.
    pub encoding: nvisy_core::content::TextEncoding,
}

/// Loader that validates and parses HTML files.
///
/// Produces a single [`HtmlHandler`] per input.
#[derive(Debug, Default)]
pub struct HtmlLoader;

#[async_trait::async_trait]
impl Loader for HtmlLoader {
    type Handler = HtmlHandler;
    type Params = HtmlParams;

    #[tracing::instrument(name = "html.decode", skip_all, fields(input_bytes, text_nodes))]
    async fn decode(
        &self,
        content: &ContentData,
        params: &Self::Params,
    ) -> Result<HtmlHandler, Error> {
        let raw = content.to_bytes();
        tracing::Span::current().record("input_bytes", raw.len());
        let text = params.encoding.decode_bytes(&raw, "html-loader")?;
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

        let source = ContentSource::new().with_parent(&content.content_source);
        let handler = HtmlHandler::new(HtmlData {
            text_nodes,
            raw: text,
        })
        .with_source(source);
        Ok(handler)
    }
}
