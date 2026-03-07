//! [`AnyDocument`]: type-erased document wrapper over all handler families.

use derive_more::From;
use nvisy_core::Error;
use nvisy_core::fs::DocumentType;
use nvisy_core::io::ContentData;

use super::Document;
#[cfg(feature = "docx")]
use crate::handler::DocxHandler;
#[cfg(feature = "pdf")]
use crate::handler::PdfHandler;
use crate::handler::{
    AnyAudio, AnyImage, AnyRich, AnyText, JpegHandler, Mp3Handler, PngHandler, TxtHandler,
    WavHandler,
};

/// A fully type-erased document that can hold any supported format.
///
/// Groups documents into four modality families:
/// - **Text**: plain text, CSV, JSON, HTML, XLSX
/// - **Image**: PNG, JPEG
/// - **Audio**: WAV, MP3
/// - **Rich**: PDF, DOCX (multi-modal documents with text + images)
#[derive(From)]
pub enum AnyDocument {
    Text(Document<AnyText>),
    Image(Document<AnyImage>),
    Audio(Document<AnyAudio>),
    Rich(Document<AnyRich>),
}

impl AnyDocument {
    /// The document type of the underlying content.
    pub fn document_type(&self) -> DocumentType {
        match self {
            Self::Text(d) => d.document_type(),
            Self::Image(d) => d.document_type(),
            Self::Audio(d) => d.document_type(),
            Self::Rich(d) => d.document_type(),
        }
    }

    /// Encode the document back to raw bytes.
    pub fn encode(&self) -> Result<ContentData, Error> {
        match self {
            Self::Text(d) => d.encode(),
            Self::Image(d) => d.encode(),
            Self::Audio(d) => d.encode(),
            Self::Rich(d) => d.encode(),
        }
    }

    /// Try to get the inner text document by reference.
    pub fn as_text(&self) -> Option<&Document<AnyText>> {
        if let Self::Text(d) = self {
            Some(d)
        } else {
            None
        }
    }

    /// Try to get the inner image document by reference.
    pub fn as_image(&self) -> Option<&Document<AnyImage>> {
        if let Self::Image(d) = self {
            Some(d)
        } else {
            None
        }
    }

    /// Try to get the inner audio document by reference.
    pub fn as_audio(&self) -> Option<&Document<AnyAudio>> {
        if let Self::Audio(d) = self {
            Some(d)
        } else {
            None
        }
    }

    /// Try to get the inner rich document by reference.
    pub fn as_rich(&self) -> Option<&Document<AnyRich>> {
        if let Self::Rich(d) = self {
            Some(d)
        } else {
            None
        }
    }

    /// Consume and return the inner text document.
    pub fn into_text(self) -> Option<Document<AnyText>> {
        if let Self::Text(d) = self {
            Some(d)
        } else {
            None
        }
    }

    /// Consume and return the inner image document.
    pub fn into_image(self) -> Option<Document<AnyImage>> {
        if let Self::Image(d) = self {
            Some(d)
        } else {
            None
        }
    }

    /// Consume and return the inner audio document.
    pub fn into_audio(self) -> Option<Document<AnyAudio>> {
        if let Self::Audio(d) = self {
            Some(d)
        } else {
            None
        }
    }

    /// Consume and return the inner rich document.
    pub fn into_rich(self) -> Option<Document<AnyRich>> {
        if let Self::Rich(d) = self {
            Some(d)
        } else {
            None
        }
    }
}

impl From<Document<TxtHandler>> for AnyDocument {
    fn from(d: Document<TxtHandler>) -> Self {
        Self::Text(d.map_handler(AnyText::from))
    }
}

impl From<Document<PngHandler>> for AnyDocument {
    fn from(d: Document<PngHandler>) -> Self {
        Self::Image(d.map_handler(AnyImage::from))
    }
}

impl From<Document<JpegHandler>> for AnyDocument {
    fn from(d: Document<JpegHandler>) -> Self {
        Self::Image(d.map_handler(AnyImage::from))
    }
}

impl From<Document<WavHandler>> for AnyDocument {
    fn from(d: Document<WavHandler>) -> Self {
        Self::Audio(d.map_handler(AnyAudio::from))
    }
}

impl From<Document<Mp3Handler>> for AnyDocument {
    fn from(d: Document<Mp3Handler>) -> Self {
        Self::Audio(d.map_handler(AnyAudio::from))
    }
}

#[cfg(feature = "pdf")]
impl From<Document<PdfHandler>> for AnyDocument {
    fn from(d: Document<PdfHandler>) -> Self {
        Self::Rich(d.map_handler(AnyRich::from))
    }
}

#[cfg(feature = "docx")]
impl From<Document<DocxHandler>> for AnyDocument {
    fn from(d: Document<DocxHandler>) -> Self {
        Self::Rich(d.map_handler(AnyRich::from))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_txt_handler() {
        let doc = Document::new(TxtHandler::new(vec!["hello".into()], false));
        let any: AnyDocument = doc.into();
        assert!(any.as_text().is_some());
        assert_eq!(any.document_type(), DocumentType::Txt);
    }

    #[test]
    fn from_png_handler() {
        let img = image::DynamicImage::new_rgb8(1, 1);
        let doc = Document::new(PngHandler::new(img));
        let any: AnyDocument = doc.into();
        assert!(any.as_image().is_some());
        assert_eq!(any.document_type(), DocumentType::Png);
    }

    #[test]
    fn from_wav_handler() {
        let doc = Document::new(WavHandler::new(bytes::Bytes::from_static(b"wav")));
        let any: AnyDocument = doc.into();
        assert!(any.as_audio().is_some());
        assert_eq!(any.document_type(), DocumentType::Wav);
    }

    #[test]
    fn into_text_returns_some() {
        let doc = Document::new(TxtHandler::new(vec![], false));
        let any: AnyDocument = doc.into();
        assert!(any.into_text().is_some());
    }

    #[test]
    fn into_wrong_variant_returns_none() {
        let doc = Document::new(TxtHandler::new(vec![], false));
        let any: AnyDocument = doc.into();
        assert!(any.into_image().is_none());
    }
}
