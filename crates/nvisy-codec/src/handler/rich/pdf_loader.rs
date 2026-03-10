//! PDF loader: parses raw PDF content into a [`RichTextHandler`].
//!
//! Text is extracted per page via [`lopdf`].  The raw bytes are
//! preserved for encoding and rendering.

use nvisy_core::io::ContentData;
use nvisy_core::path::ContentSource;

use crate::handler::{Loader, RichTextHandler};

/// Parameters for [`PdfLoader`].
#[derive(Debug, Default)]
pub struct PdfParams {
    /// Optional password for encrypted PDFs.
    pub password: Option<String>,
}

/// Loader that parses PDF files and extracts per-page text.
///
/// Produces a single [`RichTextHandler`] per input.
#[derive(Debug, Default)]
pub struct PdfLoader;

#[async_trait::async_trait]
impl Loader for PdfLoader {
    type Handler = RichTextHandler;
    type Params = PdfParams;

    #[tracing::instrument(name = "pdf.decode", skip_all, fields(input_bytes, pages))]
    async fn decode(
        &self,
        content: &ContentData,
        params: &Self::Params,
    ) -> Result<RichTextHandler, nvisy_core::Error> {
        let raw = content.to_bytes();
        tracing::Span::current().record("input_bytes", raw.len());

        let source = ContentSource::new().with_parent(&content.content_source);
        let handler =
            RichTextHandler::from_pdf(raw, params.password.as_deref())?.with_source(source);

        tracing::Span::current().record("pages", handler.page_count());

        Ok(handler)
    }
}

#[cfg(test)]
mod tests {
    use bytes::Bytes;
    use futures::StreamExt;
    use nvisy_core::fs::DocumentType;
    use nvisy_core::path::ContentSource;

    use super::*;
    use crate::handler::{Handler, TextHandler};

    fn content_from_bytes(bytes: &[u8]) -> ContentData {
        ContentData::new(ContentSource::new(), Bytes::from(bytes.to_vec()))
    }

    /// Build a minimal valid PDF with one blank page using lopdf.
    fn minimal_pdf() -> Vec<u8> {
        use lopdf::{Dictionary, Document, Object, Stream, dictionary};

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
        let err = PdfLoader
            .decode(&content, &PdfParams::default())
            .await
            .unwrap_err();
        assert!(err.to_string().contains("failed to extract text from PDF"));
    }

    #[tokio::test]
    async fn load_minimal_pdf() {
        let raw = minimal_pdf();
        let content = content_from_bytes(&raw);
        let doc = PdfLoader
            .decode(&content, &PdfParams::default())
            .await
            .unwrap();

        assert_eq!(doc.document_type(), DocumentType::Pdf);
        assert_eq!(doc.page_count(), 1);
        assert!(doc.page(0).unwrap().trim().is_empty());
    }

    #[tokio::test]
    async fn load_preserves_raw_bytes() {
        let raw = minimal_pdf();
        let content = content_from_bytes(&raw);
        let doc = PdfLoader
            .decode(&content, &PdfParams::default())
            .await
            .unwrap();

        assert_eq!(doc.raw(), &raw);
    }

    #[tokio::test]
    async fn view_spans_matches_pages() {
        let raw = minimal_pdf();
        let content = content_from_bytes(&raw);
        let doc = PdfLoader
            .decode(&content, &PdfParams::default())
            .await
            .unwrap();

        let spans: Vec<_> = doc.text_spans().await.collect().await;
        assert_eq!(spans.len(), doc.page_count());
    }
}
