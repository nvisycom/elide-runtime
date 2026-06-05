//! Plain-text loader: validates and parses raw text content into a
//! [`TxtHandler`].

use async_trait::async_trait;
use nvisy_codec::handler::Loader;
use nvisy_core::Error;
use nvisy_core::content::{ContentData, ContentSource, TextEncoding};
use nvisy_core::modality::Text;

use super::TxtHandler;

/// Loader that validates and parses plain-text files. Produces one
/// [`TxtHandler`] per input.
#[derive(Debug, Default)]
pub struct TxtLoader {
    /// Character encoding of the input bytes. Defaults to UTF-8.
    pub encoding: TextEncoding,
}

#[async_trait]
impl Loader<Text> for TxtLoader {
    type Handler = TxtHandler;

    #[tracing::instrument(name = "txt.decode", skip_all, fields(input_bytes, lines))]
    async fn decode(&self, content: ContentData) -> Result<TxtHandler, Error> {
        let parent = content.content_source;
        let raw = content.to_bytes();
        tracing::Span::current().record("input_bytes", raw.len());
        let text = self.encoding.decode_bytes(&raw, "txt-loader")?;
        let trailing_newline = text.ends_with('\n');
        let lines: Vec<String> = text.lines().map(String::from).collect();
        tracing::Span::current().record("lines", lines.len());

        let source = ContentSource::new().with_parent(&parent);
        Ok(TxtHandler::new(lines, trailing_newline).with_source(source))
    }
}

#[cfg(test)]
mod tests {
    use bytes::Bytes;
    use nvisy_codec::handler::Handler;
    use nvisy_core::Error;
    use nvisy_core::content::ContentSource;

    use super::*;

    fn content_from_str(s: &str) -> ContentData {
        ContentData::new(ContentSource::new(), Bytes::from(s.to_owned()))
    }

    #[tokio::test]
    async fn load_multiline() -> Result<(), Error> {
        let content = content_from_str("hello\nworld\n");
        let doc = TxtLoader::default().decode(content).await?;
        assert_eq!(doc.format().as_str(), "nvisy.text.txt");
        assert_eq!(doc.lines(), &["hello", "world"]);
        assert!(doc.trailing_newline());
        Ok(())
    }

    #[tokio::test]
    async fn load_no_trailing_newline() -> Result<(), Error> {
        let content = content_from_str("single line");
        let doc = TxtLoader::default().decode(content).await?;
        assert_eq!(doc.len(), 1);
        assert_eq!(doc.line(0), Some("single line"));
        assert!(!doc.trailing_newline());
        Ok(())
    }

    #[tokio::test]
    async fn load_invalid_utf8() {
        let content = ContentData::new(
            ContentSource::new(),
            Bytes::from_static(&[0xFF, 0xFE, 0x00]),
        );
        let err = TxtLoader::default().decode(content).await.unwrap_err();
        assert!(err.to_string().contains("UTF-8"));
    }
}
