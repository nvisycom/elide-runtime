//! [`AnyDocument`]: type-erased document wrapper over all handler families.

use derive_more::From;
use nvisy_core::Error;
use nvisy_core::fs::DocumentType;
use nvisy_core::io::ContentData;

#[cfg(feature = "docx")]
use crate::handler::DocxHandler;
#[cfg(feature = "pdf")]
use crate::handler::PdfHandler;
use crate::handler::{
    BoxedAudioHandler, BoxedImageHandler, AnyRich, AnyText, Handler, JpegHandler, Mp3Handler,
    PngHandler, TxtHandler, WavHandler,
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
    Text(AnyText),
    Image(BoxedImageHandler),
    Audio(BoxedAudioHandler),
    Rich(AnyRich),
}

impl AnyDocument {
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

    /// Try to get the inner text handler by reference.
    pub fn as_text(&self) -> Option<&AnyText> {
        if let Self::Text(h) = self {
            Some(h)
        } else {
            None
        }
    }

    /// Try to get the inner image handler by reference.
    pub fn as_image(&self) -> Option<&BoxedImageHandler> {
        if let Self::Image(h) = self {
            Some(h)
        } else {
            None
        }
    }

    /// Try to get the inner audio handler by reference.
    pub fn as_audio(&self) -> Option<&BoxedAudioHandler> {
        if let Self::Audio(h) = self {
            Some(h)
        } else {
            None
        }
    }

    /// Try to get the inner rich handler by reference.
    pub fn as_rich(&self) -> Option<&AnyRich> {
        if let Self::Rich(h) = self {
            Some(h)
        } else {
            None
        }
    }

    /// Consume and return the inner text handler.
    pub fn into_text(self) -> Option<AnyText> {
        if let Self::Text(h) = self {
            Some(h)
        } else {
            None
        }
    }

    /// Consume and return the inner image handler.
    pub fn into_image(self) -> Option<BoxedImageHandler> {
        if let Self::Image(h) = self {
            Some(h)
        } else {
            None
        }
    }

    /// Consume and return the inner audio handler.
    pub fn into_audio(self) -> Option<BoxedAudioHandler> {
        if let Self::Audio(h) = self {
            Some(h)
        } else {
            None
        }
    }

    /// Consume and return the inner rich handler.
    pub fn into_rich(self) -> Option<AnyRich> {
        if let Self::Rich(h) = self {
            Some(h)
        } else {
            None
        }
    }
}

impl From<TxtHandler> for AnyDocument {
    fn from(h: TxtHandler) -> Self {
        Self::Text(AnyText::from(h))
    }
}

impl From<PngHandler> for AnyDocument {
    fn from(h: PngHandler) -> Self {
        Self::Image(BoxedImageHandler::from(h))
    }
}

impl From<JpegHandler> for AnyDocument {
    fn from(h: JpegHandler) -> Self {
        Self::Image(BoxedImageHandler::from(h))
    }
}

impl From<WavHandler> for AnyDocument {
    fn from(h: WavHandler) -> Self {
        Self::Audio(BoxedAudioHandler::from(h))
    }
}

impl From<Mp3Handler> for AnyDocument {
    fn from(h: Mp3Handler) -> Self {
        Self::Audio(BoxedAudioHandler::from(h))
    }
}

#[cfg(feature = "pdf")]
impl From<PdfHandler> for AnyDocument {
    fn from(h: PdfHandler) -> Self {
        Self::Rich(AnyRich::from(h))
    }
}

#[cfg(feature = "docx")]
impl From<DocxHandler> for AnyDocument {
    fn from(h: DocxHandler) -> Self {
        Self::Rich(AnyRich::from(h))
    }
}

#[cfg(test)]
mod tests {
    use nvisy_core::fs::{AudioFormat, ImageFormat, TextFormat};

    use super::*;

    #[test]
    fn from_txt_handler() {
        let handler = TxtHandler::new(vec!["hello".into()], false);
        let any: AnyDocument = handler.into();
        assert!(any.as_text().is_some());
        assert_eq!(any.document_type(), DocumentType::Text(TextFormat::Txt));
    }

    #[test]
    fn from_png_handler() {
        let img = image::DynamicImage::new_rgb8(1, 1);
        let handler = PngHandler::new(img);
        let any: AnyDocument = handler.into();
        assert!(any.as_image().is_some());
        assert_eq!(any.document_type(), DocumentType::Image(ImageFormat::Png));
    }

    #[test]
    fn from_wav_handler() {
        let handler = WavHandler::new(bytes::Bytes::from_static(b"wav"));
        let any: AnyDocument = handler.into();
        assert!(any.as_audio().is_some());
        assert_eq!(any.document_type(), DocumentType::Audio(AudioFormat::Wav));
    }

    #[test]
    fn into_text_returns_some() {
        let handler = TxtHandler::new(vec![], false);
        let any: AnyDocument = handler.into();
        assert!(any.into_text().is_some());
    }

    #[test]
    fn into_wrong_variant_returns_none() {
        let handler = TxtHandler::new(vec![], false);
        let any: AnyDocument = handler.into();
        assert!(any.into_image().is_none());
    }
}
