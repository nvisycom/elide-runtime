//! PDF loader: parses raw PDF content into a [`PdfHandler`]. Text is
//! extracted per page via [`lopdf`]; the raw bytes are preserved for
//! encoding and rendering.
//!
//! [`lopdf`]: https://docs.rs/lopdf

use async_trait::async_trait;
use nvisy_codec::handler::Loader;
use nvisy_core::Error;
use nvisy_core::content::{ContentData, ContentSource};
use nvisy_core::modality::Text;

use super::PdfHandler;

/// Loader for PDF files. Produces one [`PdfHandler`] per input.
#[derive(Debug, Default)]
pub struct PdfLoader {
    /// Optional password for encrypted PDFs.
    pub password: Option<String>,
}

#[async_trait]
impl Loader<Text> for PdfLoader {
    type Handler = PdfHandler;

    #[tracing::instrument(name = "pdf.decode", skip_all, fields(input_bytes))]
    async fn decode(&self, content: ContentData) -> Result<PdfHandler, Error> {
        let parent = content.content_source;
        let raw = content.to_bytes();
        tracing::Span::current().record("input_bytes", raw.len());
        let source = ContentSource::new().with_parent(&parent);
        Ok(PdfHandler::from_pdf(raw, self.password.as_deref())?.with_source(source))
    }
}

#[cfg(test)]
mod tests {
    use bytes::Bytes;
    use lopdf::{Dictionary, Document, Object, Stream, dictionary};
    use nvisy_codec::core::Handle;
    use nvisy_codec::handler::Handler;
    use nvisy_core::content::ContentSource;

    use super::*;

    fn content_from_bytes(bytes: &[u8]) -> ContentData {
        ContentData::new(ContentSource::new(), Bytes::from(bytes.to_vec()))
    }

    fn minimal_pdf() -> Vec<u8> {
        let mut doc = Document::with_version("1.5");
        let pages_id = doc.new_object_id();
        let page_id = doc.new_object_id();
        let content_id = doc.add_object(Stream::new(Dictionary::new(), Vec::new()));
        doc.set_object(
            page_id,
            Object::Dictionary(dictionary! {
                "Type" => "Page",
                "Parent" => pages_id,
                "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
                "Contents" => content_id,
            }),
        );
        doc.set_object(
            pages_id,
            Object::Dictionary(dictionary! {
                "Type" => "Pages",
                "Kids" => vec![page_id.into()],
                "Count" => 1,
            }),
        );
        let catalog_id = doc.add_object(dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
        });
        doc.trailer.set("Root", catalog_id);
        let mut buf = Vec::new();
        doc.save_to(&mut buf).expect("failed to save minimal PDF");
        buf
    }

    #[tokio::test]
    async fn load_invalid_pdf_returns_error() {
        let content = content_from_bytes(b"not a pdf");
        let err = PdfLoader::default().decode(content).await.unwrap_err();
        assert!(err.to_string().contains("failed to extract text from PDF"));
    }

    #[tokio::test]
    async fn load_minimal_pdf() {
        let raw = minimal_pdf();
        let content = content_from_bytes(&raw);
        let mut doc = PdfLoader::default().decode(content).await.unwrap();
        assert_eq!(doc.format().as_str(), "nvisy.rich.pdf");
        let chunk = doc.next_chunk().await.unwrap().expect("one page");
        assert!(chunk.data.as_str().trim().is_empty());
    }
}
