//! Unified document representation.

mod span;
mod stream;

pub use span::Span;
pub use stream::SpanStream;

use derive_more::{From, IsVariant, TryInto};
use nvisy_core::Error;
use nvisy_core::fs::DocumentType;
use nvisy_core::io::ContentData;

use crate::handler::{
    AnyText, BoxedAudioHandler, BoxedImageHandler, BoxedRichHandler, Handler,
};

/// A fully type-erased document that can hold any supported format.
///
/// Groups documents into four modality families:
/// - **Text**: plain text, CSV, JSON, HTML, XLSX
/// - **Image**: PNG, JPEG
/// - **Audio**: WAV, MP3
/// - **Rich**: PDF, DOCX (multi-modal documents with text + images)
///
/// Use [`From`] conversions to wrap concrete handlers or boxed wrappers.
/// Use [`IsVariant`]-generated `is_*` methods or [`TryInto`] to
/// extract the inner handler.
#[derive(From, IsVariant, TryInto)]
pub enum Document {
    Text(AnyText),
    Image(BoxedImageHandler),
    Audio(BoxedAudioHandler),
    Rich(BoxedRichHandler),
}

impl Document {
    /// The document type of the underlying content.
    pub fn document_type(&self) -> DocumentType {
        match self {
            Self::Text(h) => h.document_type(),
            Self::Image(h) => h.document_type(),
            Self::Audio(h) => h.document_type(),
            Self::Rich(h) => h.document_type(),
        }
    }

    /// Encode the document back to raw bytes.
    pub fn encode(&self) -> Result<ContentData, Error> {
        match self {
            Self::Text(h) => h.encode(),
            Self::Image(h) => h.encode(),
            Self::Audio(h) => h.encode(),
            Self::Rich(h) => h.encode(),
        }
    }
}

#[cfg(test)]
mod tests {
    use nvisy_core::fs::{AudioFormat, ImageFormat, TextFormat};

    use super::*;
    use crate::handler::{Mp3Handler, PngHandler, TxtHandler, WavHandler};

    #[test]
    fn from_txt_handler() {
        let doc = Document::from(AnyText::from(TxtHandler::new(vec!["hello".into()], false)));
        assert!(doc.is_text());
        assert_eq!(doc.document_type(), DocumentType::Text(TextFormat::Txt));
    }

    #[test]
    fn from_png_handler() {
        let img = image::DynamicImage::new_rgb8(1, 1);
        let doc = Document::from(BoxedImageHandler::from(PngHandler::new(img)));
        assert!(doc.is_image());
        assert_eq!(doc.document_type(), DocumentType::Image(ImageFormat::Png));
    }

    #[test]
    fn from_wav_handler() {
        let doc = Document::from(BoxedAudioHandler::from(WavHandler::new(
            bytes::Bytes::from_static(b"wav"),
        )));
        assert!(doc.is_audio());
        assert_eq!(doc.document_type(), DocumentType::Audio(AudioFormat::Wav));
    }

    #[test]
    fn try_into_text() {
        let doc = Document::from(AnyText::from(TxtHandler::new(vec![], false)));
        let text: Result<AnyText, _> = doc.try_into();
        assert!(text.is_ok());
    }

    #[test]
    fn try_into_wrong_variant() {
        let doc = Document::from(AnyText::from(TxtHandler::new(vec![], false)));
        let image: Result<BoxedImageHandler, _> = doc.try_into();
        assert!(image.is_err());
    }
}
