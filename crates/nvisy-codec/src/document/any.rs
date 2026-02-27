//! [`AnyDocument`]: type-erased wrapper over all supported document types.

use nvisy_core::Error;
use nvisy_core::fs::DocumentType;

use crate::handler::Handler;

use crate::document::Document;
use crate::handler::{
    TxtHandler, CsvHandler, JsonHandler,
    AnyImage, AnyAudio,
};
#[cfg(feature = "html")]
use crate::handler::HtmlHandler;
#[cfg(feature = "pdf")]
use crate::handler::PdfHandler;
#[cfg(feature = "docx")]
use crate::handler::DocxHandler;
#[cfg(feature = "xlsx")]
use crate::handler::XlsxHandler;

/// A type-erased document that can hold any supported format.
///
/// Produced by [`UniversalLoader`](super::UniversalLoader) when the
/// caller does not know the format ahead of time.
pub enum AnyDocument {
    Txt(Document<TxtHandler>),
    Csv(Document<CsvHandler>),
    Json(Document<JsonHandler>),
    #[cfg(feature = "html")]
    Html(Document<HtmlHandler>),
    Image(Document<AnyImage>),
    Audio(Document<AnyAudio>),
    #[cfg(feature = "pdf")]
    Pdf(Document<PdfHandler>),
    #[cfg(feature = "docx")]
    Docx(Document<DocxHandler>),
    #[cfg(feature = "xlsx")]
    Xlsx(Document<XlsxHandler>),
}

impl AnyDocument {
    /// The document type of the inner document.
    pub fn document_type(&self) -> DocumentType {
        match self {
            Self::Txt(d) => d.document_type(),
            Self::Csv(d) => d.document_type(),
            Self::Json(d) => d.document_type(),
            #[cfg(feature = "html")]
            Self::Html(d) => d.document_type(),
            Self::Image(d) => d.document_type(),
            Self::Audio(d) => d.document_type(),
            #[cfg(feature = "pdf")]
            Self::Pdf(d) => d.document_type(),
            #[cfg(feature = "docx")]
            Self::Docx(d) => d.document_type(),
            #[cfg(feature = "xlsx")]
            Self::Xlsx(d) => d.document_type(),
        }
    }

    /// Encode the inner document back to raw bytes.
    pub fn encode(&self) -> Result<Vec<u8>, Error> {
        match self {
            Self::Txt(d) => d.encode(),
            Self::Csv(d) => d.encode(),
            Self::Json(d) => d.encode(),
            #[cfg(feature = "html")]
            Self::Html(d) => d.encode(),
            Self::Image(d) => d.encode(),
            Self::Audio(d) => d.encode(),
            #[cfg(feature = "pdf")]
            Self::Pdf(d) => d.encode(),
            #[cfg(feature = "docx")]
            Self::Docx(d) => d.encode(),
            #[cfg(feature = "xlsx")]
            Self::Xlsx(d) => d.encode(),
        }
    }

    /// Try to get the inner `Document<TxtHandler>` by reference.
    pub fn as_txt(&self) -> Option<&Document<TxtHandler>> {
        if let Self::Txt(d) = self { Some(d) } else { None }
    }

    /// Consume and return the inner `Document<TxtHandler>`.
    pub fn into_txt(self) -> Option<Document<TxtHandler>> {
        if let Self::Txt(d) = self { Some(d) } else { None }
    }

    /// Try to get the inner `Document<CsvHandler>` by reference.
    pub fn as_csv(&self) -> Option<&Document<CsvHandler>> {
        if let Self::Csv(d) = self { Some(d) } else { None }
    }

    /// Consume and return the inner `Document<CsvHandler>`.
    pub fn into_csv(self) -> Option<Document<CsvHandler>> {
        if let Self::Csv(d) = self { Some(d) } else { None }
    }

    /// Try to get the inner `Document<JsonHandler>` by reference.
    pub fn as_json(&self) -> Option<&Document<JsonHandler>> {
        if let Self::Json(d) = self { Some(d) } else { None }
    }

    /// Consume and return the inner `Document<JsonHandler>`.
    pub fn into_json(self) -> Option<Document<JsonHandler>> {
        if let Self::Json(d) = self { Some(d) } else { None }
    }

    /// Try to get the inner `Document<AnyImage>` by reference.
    pub fn as_image(&self) -> Option<&Document<AnyImage>> {
        if let Self::Image(d) = self { Some(d) } else { None }
    }

    /// Consume and return the inner `Document<AnyImage>`.
    pub fn into_image(self) -> Option<Document<AnyImage>> {
        if let Self::Image(d) = self { Some(d) } else { None }
    }

    /// Try to get the inner `Document<AnyAudio>` by reference.
    pub fn as_audio(&self) -> Option<&Document<AnyAudio>> {
        if let Self::Audio(d) = self { Some(d) } else { None }
    }

    /// Consume and return the inner `Document<AnyAudio>`.
    pub fn into_audio(self) -> Option<Document<AnyAudio>> {
        if let Self::Audio(d) = self { Some(d) } else { None }
    }

    /// Try to get the inner `Document<HtmlHandler>` by reference.
    #[cfg(feature = "html")]
    pub fn as_html(&self) -> Option<&Document<HtmlHandler>> {
        if let Self::Html(d) = self { Some(d) } else { None }
    }

    /// Consume and return the inner `Document<HtmlHandler>`.
    #[cfg(feature = "html")]
    pub fn into_html(self) -> Option<Document<HtmlHandler>> {
        if let Self::Html(d) = self { Some(d) } else { None }
    }

    /// Try to get the inner `Document<PdfHandler>` by reference.
    #[cfg(feature = "pdf")]
    pub fn as_pdf(&self) -> Option<&Document<PdfHandler>> {
        if let Self::Pdf(d) = self { Some(d) } else { None }
    }

    /// Consume and return the inner `Document<PdfHandler>`.
    #[cfg(feature = "pdf")]
    pub fn into_pdf(self) -> Option<Document<PdfHandler>> {
        if let Self::Pdf(d) = self { Some(d) } else { None }
    }

    /// Try to get the inner `Document<DocxHandler>` by reference.
    #[cfg(feature = "docx")]
    pub fn as_docx(&self) -> Option<&Document<DocxHandler>> {
        if let Self::Docx(d) = self { Some(d) } else { None }
    }

    /// Consume and return the inner `Document<DocxHandler>`.
    #[cfg(feature = "docx")]
    pub fn into_docx(self) -> Option<Document<DocxHandler>> {
        if let Self::Docx(d) = self { Some(d) } else { None }
    }

    /// Try to get the inner `Document<XlsxHandler>` by reference.
    #[cfg(feature = "xlsx")]
    pub fn as_xlsx(&self) -> Option<&Document<XlsxHandler>> {
        if let Self::Xlsx(d) = self { Some(d) } else { None }
    }

    /// Consume and return the inner `Document<XlsxHandler>`.
    #[cfg(feature = "xlsx")]
    pub fn into_xlsx(self) -> Option<Document<XlsxHandler>> {
        if let Self::Xlsx(d) = self { Some(d) } else { None }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handler::{TxtHandler, WavHandler};

    #[test]
    fn document_type_returns_correct_variant() {
        let handler = TxtHandler::new(vec!["hello".into()], true);
        let doc = AnyDocument::Txt(Document::new(handler));
        assert_eq!(doc.document_type(), DocumentType::Txt);
    }

    #[test]
    fn encode_delegates_to_inner_handler() {
        let handler = TxtHandler::new(vec!["hello".into()], true);
        let doc = AnyDocument::Txt(Document::new(handler));
        let bytes = doc.encode().unwrap();
        assert_eq!(bytes, b"hello\n");
    }

    #[test]
    fn as_txt_returns_some_for_txt() {
        let handler = TxtHandler::new(vec!["test".into()], false);
        let doc = AnyDocument::Txt(Document::new(handler));
        assert!(doc.as_txt().is_some());
    }

    #[test]
    fn as_txt_returns_none_for_other() {
        let handler = WavHandler::new(bytes::Bytes::from_static(b"wav"));
        let doc = AnyDocument::Audio(Document::new(AnyAudio::from(handler)));
        assert!(doc.as_txt().is_none());
    }

    #[test]
    fn into_txt_consumes_and_returns() {
        let handler = TxtHandler::new(vec!["data".into()], false);
        let doc = AnyDocument::Txt(Document::new(handler));
        let inner = doc.into_txt().unwrap();
        assert_eq!(inner.handler().lines(), &["data"]);
    }

    #[test]
    fn audio_variant_holds_any_audio() {
        let handler = WavHandler::new(bytes::Bytes::from_static(b"wav"));
        let doc = AnyDocument::Audio(Document::new(AnyAudio::from(handler)));
        assert_eq!(doc.document_type(), DocumentType::Wav);
        assert!(doc.as_audio().is_some());
    }

    #[test]
    fn image_variant_holds_any_image() {
        use crate::handler::PngHandler;
        let handler = PngHandler::new(image::DynamicImage::new_rgb8(1, 1));
        let doc = AnyDocument::Image(Document::new(AnyImage::from(handler)));
        assert_eq!(doc.document_type(), DocumentType::Png);
        assert!(doc.as_image().is_some());
    }
}
