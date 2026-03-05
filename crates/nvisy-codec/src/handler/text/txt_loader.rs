//! Plain-text loader: validates and parses raw text content into a
//! [`Document<TxtHandler>`].
//!
//! The loader splits the input into lines and records whether the
//! source ended with a trailing newline so the file can be
//! reconstructed after edits.

use nvisy_core::Error;
use nvisy_core::io::{ContentData, TextEncoding};

use crate::document::Document;
use crate::handler::{Loader, TxtHandler};

/// Parameters for [`TxtLoader`].
#[derive(Debug, Default)]
pub struct TxtParams {
    /// Character encoding of the input bytes.
    pub encoding: TextEncoding,
}

/// Loader that validates and parses plain-text files.
///
/// Produces a single [`Document<TxtHandler>`] per input.
#[derive(Debug)]
pub struct TxtLoader;

#[async_trait::async_trait]
impl Loader for TxtLoader {
    type Handler = TxtHandler;
    type Params = TxtParams;

    #[tracing::instrument(name = "txt.decode", skip_all, fields(input_bytes, lines))]
    async fn decode(
        &self,
        content: &ContentData,
        params: &Self::Params,
    ) -> Result<Document<TxtHandler>, Error> {
        let raw = content.to_bytes();
        tracing::Span::current().record("input_bytes", raw.len());
        let text = params.encoding.decode_bytes(&raw, "txt-loader")?;
        let trailing_newline = text.ends_with('\n');
        let lines: Vec<String> = text.lines().map(String::from).collect();
        tracing::Span::current().record("lines", lines.len());

        let handler = TxtHandler::new(lines, trailing_newline).with_source(content.content_source);
        let doc = Document::new(handler).with_parent(content);
        Ok(doc)
    }
}

#[cfg(test)]
mod tests {
    use bytes::Bytes;
    use futures::StreamExt;
    use nvisy_core::Error;
    use nvisy_core::fs::DocumentType;
    use nvisy_core::path::ContentSource;

    use super::*;

    fn content_from_str(s: &str) -> ContentData {
        ContentData::new(ContentSource::new(), Bytes::from(s.to_owned()))
    }

    #[tokio::test]
    async fn load_multiline() -> Result<(), Error> {
        let content = content_from_str("hello\nworld\n");
        let doc = TxtLoader.decode(&content, &TxtParams::default()).await?;

        assert_eq!(doc.document_type(), DocumentType::Txt);
        assert_eq!(doc.lines(), &["hello", "world"]);
        assert!(doc.trailing_newline());
        Ok(())
    }

    #[tokio::test]
    async fn load_no_trailing_newline() -> Result<(), Error> {
        let content = content_from_str("single line");
        let doc = TxtLoader.decode(&content, &TxtParams::default()).await?;

        assert_eq!(doc.len(), 1);
        assert_eq!(doc.line(0), Some("single line"));
        assert!(!doc.trailing_newline());
        Ok(())
    }

    #[tokio::test]
    async fn load_preserves_spans_through_round_trip() -> Result<(), Error> {
        let content = content_from_str("Alice\nBob\nCharlie\n");
        let doc = TxtLoader.decode(&content, &TxtParams::default()).await?;

        let spans: Vec<_> = doc.text_spans().await.collect().await;
        assert_eq!(spans.len(), 3);
        assert_eq!(spans[0].data, "Alice");
        assert_eq!(spans[1].data, "Bob");
        assert_eq!(spans[2].data, "Charlie");
        Ok(())
    }

    #[tokio::test]
    async fn load_invalid_utf8() {
        let content = ContentData::new(
            ContentSource::new(),
            Bytes::from_static(&[0xFF, 0xFE, 0x00]),
        );
        let err = TxtLoader
            .decode(&content, &TxtParams::default())
            .await
            .unwrap_err();
        assert!(err.to_string().contains("UTF-8"));
    }
}
