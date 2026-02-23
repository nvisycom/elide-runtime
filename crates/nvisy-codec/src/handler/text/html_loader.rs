//! HTML loader: validates and parses raw HTML content into a
//! [`Document<HtmlHandler>`].
//!
//! The loader parses the input using [`scraper`], extracts text nodes
//! in document order, and produces a handler backed by those nodes.

use scraper::Html;

use nvisy_core::Error;
use nvisy_core::io::ContentData;

use crate::document::Document;
use crate::handler::{Loader, HtmlData, HtmlHandler};

/// Parameters for [`HtmlLoader`].
#[derive(Debug, Default)]
pub struct HtmlParams {
    /// Character encoding of the input bytes.
    pub encoding: nvisy_core::data::TextEncoding,
}

/// Loader that validates and parses HTML files.
///
/// Produces a single [`Document<HtmlHandler>`] per input.
#[derive(Debug)]
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
    ) -> Result<Vec<Document<HtmlHandler>>, Error> {
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

        let handler = HtmlHandler::new(HtmlData {
            text_nodes,
            raw: text,
        });
        let doc = Document::new(handler).with_parent(content);
        Ok(vec![doc])
    }
}
