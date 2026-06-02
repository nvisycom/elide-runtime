//! Markdown loader: validates and parses raw Markdown content into a
//! [`TxtHandler`].
//!
//! Markdown is structurally line-based text. The loader reuses
//! [`TxtHandler`] for span access and editing; the format distinction
//! is carried by the [`DocumentType`] so downstream operations can
//! apply Markdown-aware processing when needed.
//!
//! [`DocumentType`]: nvisy_core::content::DocumentType

use nvisy_codec::handler::Loader;
use nvisy_core::Error;
use nvisy_core::content::{ContentData, ContentSource, TextEncoding};

use super::TxtHandler;

/// Parameters for [`MarkdownLoader`].
#[derive(Debug, Default)]
pub struct MarkdownParams {
    /// Character encoding of the input bytes.
    pub encoding: TextEncoding,
}

/// Loader that validates and parses Markdown files.
///
/// Produces a [`TxtHandler`] per input.
#[derive(Debug, Default)]
pub struct MarkdownLoader;

#[async_trait::async_trait]
impl Loader for MarkdownLoader {
    type Handler = TxtHandler;
    type Params = MarkdownParams;

    #[tracing::instrument(name = "markdown.decode", skip_all, fields(input_bytes, lines))]
    async fn decode(
        &self,
        content: &ContentData,
        params: &Self::Params,
    ) -> Result<TxtHandler, Error> {
        let raw = content.to_bytes();
        tracing::Span::current().record("input_bytes", raw.len());
        let text = params.encoding.decode_bytes(&raw, "markdown-loader")?;
        let trailing_newline = text.ends_with('\n');
        let lines: Vec<String> = text.lines().map(String::from).collect();
        tracing::Span::current().record("lines", lines.len());

        let source = ContentSource::new().with_parent(&content.content_source);
        let handler = TxtHandler::new(lines, trailing_newline).with_source(source);
        Ok(handler)
    }
}
