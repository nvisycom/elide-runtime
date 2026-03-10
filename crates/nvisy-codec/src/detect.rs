//! Convenience decoding from [`ContentData`] to [`Document`].
//!
//! Detection of the content type is delegated to
//! [`ContentData::infer_document_type`], which uses magic-byte sniffing
//! from `nvisy-core`. This module provides [`decode`] to go one step
//! further: detect the format, pick the right loader, and return a
//! fully parsed [`Document`].

use nvisy_core::fs::{AudioFormat, DocumentType, ImageFormat};
use nvisy_core::io::ContentData;
use nvisy_core::Error;

use crate::document::Document;
use crate::handler::{
    BoxedAudioHandler, BoxedImageHandler, JpegLoader, JpegParams, Loader, Mp3Loader, Mp3Params,
    PngLoader, PngParams, WavLoader, WavParams,
};

/// Decode [`ContentData`] into a [`Document`] using default parameters.
///
/// Detects the document type via
/// [`ContentData::infer_document_type`], then dispatches to the
/// appropriate loader. Only formats with magic-byte signatures are
/// supported — text formats (TXT, CSV, JSON) lack magic bytes and must
/// be loaded explicitly via their respective loaders.
///
/// # Errors
///
/// Returns an error if:
/// - The content type cannot be detected from magic bytes.
/// - The detected format has no corresponding loader (e.g. GIF, TIFF).
/// - The loader itself fails to decode the content.
///
/// # Example
///
/// ```ignore
/// use nvisy_core::io::ContentData;
///
/// let content = ContentData::from(png_bytes);
/// let document = decode(&content).await?;
/// ```
pub async fn decode(content: &ContentData) -> Result<Document, Error> {
    let doc_type = content.infer_document_type().ok_or_else(|| {
        Error::validation(
            "unable to detect document type from content",
            "detect::decode",
        )
    })?;

    match doc_type {
        DocumentType::Image(ImageFormat::Png) => {
            let handler = PngLoader.decode(content, &PngParams).await?;
            Ok(Document::from(BoxedImageHandler::from(handler)))
        }
        DocumentType::Image(ImageFormat::Jpeg) => {
            let handler = JpegLoader.decode(content, &JpegParams).await?;
            Ok(Document::from(BoxedImageHandler::from(handler)))
        }
        DocumentType::Audio(AudioFormat::Wav) => {
            let handler = WavLoader.decode(content, &WavParams).await?;
            Ok(Document::from(BoxedAudioHandler::from(handler)))
        }
        DocumentType::Audio(AudioFormat::Mp3) => {
            let handler = Mp3Loader.decode(content, &Mp3Params).await?;
            Ok(Document::from(BoxedAudioHandler::from(handler)))
        }
        DocumentType::Pdf => {
            #[cfg(feature = "pdf")]
            {
                use crate::handler::{BoxedRichHandler, PdfLoader, PdfParams};
                let handler = PdfLoader.decode(content, &PdfParams::default()).await?;
                Ok(Document::from(BoxedRichHandler::from(handler)))
            }
            #[cfg(not(feature = "pdf"))]
            {
                Err(Error::validation(
                    "PDF support requires the \"pdf\" feature",
                    "detect::decode",
                ))
            }
        }
        _ => Err(Error::validation(
            format!("no loader available for detected type: {doc_type}"),
            "detect::decode",
        )),
    }
}

#[cfg(test)]
mod tests {
    use nvisy_core::fs::{AudioFormat, ImageFormat};
    use nvisy_core::io::ContentData;

    use super::*;

    #[test]
    fn infer_png() {
        let png = ContentData::from(vec![
            0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A,
            0x00, 0x00, 0x00, 0x0D,
            0x49, 0x48, 0x44, 0x52,
        ]);
        assert_eq!(
            png.infer_document_type(),
            Some(DocumentType::Image(ImageFormat::Png)),
        );
    }

    #[test]
    fn infer_jpeg() {
        let jpeg = ContentData::from(vec![0xFF, 0xD8, 0xFF, 0xE0]);
        assert_eq!(
            jpeg.infer_document_type(),
            Some(DocumentType::Image(ImageFormat::Jpeg)),
        );
    }

    #[test]
    fn infer_wav() {
        let mut wav = [0u8; 12];
        wav[..4].copy_from_slice(b"RIFF");
        wav[8..12].copy_from_slice(b"WAVE");
        let wav = ContentData::from(wav.to_vec());
        assert_eq!(
            wav.infer_document_type(),
            Some(DocumentType::Audio(AudioFormat::Wav)),
        );
    }

    #[test]
    fn infer_mp3() {
        let mp3 = ContentData::from(vec![0x49, 0x44, 0x33]); // "ID3"
        assert_eq!(
            mp3.infer_document_type(),
            Some(DocumentType::Audio(AudioFormat::Mp3)),
        );
    }

    #[test]
    fn infer_pdf() {
        let pdf = ContentData::from(b"%PDF-1.4".to_vec());
        assert_eq!(pdf.infer_document_type(), Some(DocumentType::Pdf));
    }

    #[test]
    fn infer_unknown() {
        assert_eq!(ContentData::from("hello world").infer_document_type(), None);
        assert_eq!(ContentData::from("").infer_document_type(), None);
    }

    #[test]
    fn infer_respects_explicit_mime() {
        let content = ContentData::from("not really json")
            .with_content_type("application/json");
        assert_eq!(
            content.infer_document_type(),
            Some(DocumentType::Text(nvisy_core::fs::TextFormat::Json)),
        );
    }
}
