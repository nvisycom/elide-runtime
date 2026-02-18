//! Plain-text loader — validates and parses raw text content into a
//! [`Document<TxtHandler>`].
//!
//! The loader splits the input into lines and records whether the
//! source ended with a trailing newline so the file can be
//! reconstructed after edits.

use nvisy_core::error::Error;
use nvisy_core::io::ContentData;

use crate::document::Document;
use crate::handler::{Loader, TxtHandler};

/// Parameters for [`TxtLoader`].
#[derive(Debug, Default)]
pub struct TxtParams {
    /// Character encoding of the input bytes.
    pub encoding: nvisy_core::data::TextEncoding,
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

    async fn decode(
        &self,
        content: &ContentData,
        params: &Self::Params,
    ) -> Result<Vec<Document<TxtHandler>>, Error> {
        let raw = content.to_bytes();
        let text = params.encoding.decode_bytes(&raw, "txt-loader")?;
        let trailing_newline = text.ends_with('\n');
        let lines = text.lines().map(String::from).collect();

        let handler = TxtHandler::new(lines, trailing_newline);
        let doc = Document::new(handler).with_parent(content);
        Ok(vec![doc])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handler::Handler;
    use bytes::Bytes;
    use futures::StreamExt;
    use nvisy_core::path::ContentSource;
    use nvisy_core::fs::DocumentType;

    fn content_from_str(s: &str) -> ContentData {
        ContentData::new(ContentSource::new(), Bytes::from(s.to_owned()))
    }

    #[tokio::test]
    async fn load_multiline() {
        let content = content_from_str("hello\nworld\n");
        let docs = TxtLoader
            .decode(&content, &TxtParams::default())
            .await
            .unwrap();

        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].document_type(), DocumentType::Txt);

        let h = docs[0].handler();
        assert_eq!(h.lines(), &["hello", "world"]);
        assert!(h.trailing_newline());
    }

    #[tokio::test]
    async fn load_no_trailing_newline() {
        let content = content_from_str("single line");
        let docs = TxtLoader
            .decode(&content, &TxtParams::default())
            .await
            .unwrap();

        let h = docs[0].handler();
        assert_eq!(h.len(), 1);
        assert_eq!(h.line(0), Some("single line"));
        assert!(!h.trailing_newline());
    }

    #[tokio::test]
    async fn load_preserves_spans_through_round_trip() {
        let content = content_from_str("Alice\nBob\nCharlie\n");
        let docs = TxtLoader
            .decode(&content, &TxtParams::default())
            .await
            .unwrap();

        let spans: Vec<_> = docs[0].handler().view_spans().await.collect().await;
        assert_eq!(spans.len(), 3);
        assert_eq!(spans[0].data, "Alice");
        assert_eq!(spans[1].data, "Bob");
        assert_eq!(spans[2].data, "Charlie");
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
