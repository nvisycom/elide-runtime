//! Markdown loader: validates and parses raw Markdown content into a
//! [`TxtHandler`].
//!
//! Markdown is structurally line-based text. The loader reuses
//! [`TxtHandler`] for span access and editing; the format distinction
//! is carried by [`FORMAT_ID`] on the [`Format`] descriptor so
//! downstream code can apply Markdown-aware processing when needed.

use std::sync::Arc;

use async_trait::async_trait;
use nvisy_codec::handler::Loader;
use nvisy_codec::{Format, FormatId, LoaderAdapter};
use nvisy_core::Error;
use nvisy_core::content::{ContentData, ContentSource, TextEncoding};
use nvisy_core::modality::{ModalityKind, Text};

use super::TxtHandler;

/// Stable [`FormatId`] for the Markdown codec.
pub const FORMAT_ID: FormatId = FormatId::from_static("nvisy.text.markdown");

/// [`Format`] descriptor registered into [`nvisy_codec::CodecRegistry`].
pub fn format() -> Format {
    Format {
        id: FORMAT_ID.clone(),
        modality: ModalityKind::Text,
        extensions: vec!["md".into(), "markdown".into()],
        content_types: vec!["text/markdown".into()],
        loader: Arc::new(LoaderAdapter::new(MarkdownLoader::default())),
    }
}

/// Loader for Markdown files. Produces a [`TxtHandler`] per input.
#[derive(Debug, Default)]
pub struct MarkdownLoader {
    /// Character encoding of the input bytes. Defaults to UTF-8.
    pub encoding: TextEncoding,
}

#[async_trait]
impl Loader<Text> for MarkdownLoader {
    type Handler = TxtHandler;

    #[tracing::instrument(name = "markdown.decode", skip_all, fields(input_bytes, lines))]
    async fn decode(&self, content: ContentData) -> Result<TxtHandler, Error> {
        let parent = content.content_source;
        let raw = content.to_bytes();
        tracing::Span::current().record("input_bytes", raw.len());
        let text = self.encoding.decode_bytes(&raw, "markdown-loader")?;
        let trailing_newline = text.ends_with('\n');
        let lines: Vec<String> = text.lines().map(String::from).collect();
        tracing::Span::current().record("lines", lines.len());

        let source = ContentSource::new().with_parent(&parent);
        Ok(TxtHandler::new(lines, trailing_newline).with_source(source))
    }
}
