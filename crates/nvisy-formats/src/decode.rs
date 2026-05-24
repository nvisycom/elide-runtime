//! [`decode`]: dispatch a [`Content`] to the appropriate loader by
//! its detected document type and return a [`ContentHandle`].

use nvisy_codec::ContentHandle;
#[cfg(feature = "audio")]
use nvisy_codec::handler::BoxedAudioHandler;
#[cfg(feature = "image")]
use nvisy_codec::handler::BoxedImageHandler;
#[cfg(feature = "rich")]
use nvisy_codec::handler::BoxedRichHandler;
#[cfg(feature = "tabular")]
use nvisy_codec::handler::BoxedTabularHandler;
#[cfg(any(feature = "txt", feature = "json", feature = "markdown", feature = "html"))]
use nvisy_codec::handler::BoxedTextHandler;
use nvisy_codec::handler::Loader;
use nvisy_core::Error;
use nvisy_core::content::{Content, ContentData};
use nvisy_core::media::DocumentType;
#[cfg(any(feature = "wav", feature = "mp3"))]
use nvisy_core::media::AudioFormat;
#[cfg(any(feature = "png", feature = "jpeg", feature = "tiff"))]
use nvisy_core::media::ImageFormat;
#[cfg(any(feature = "csv", feature = "xlsx"))]
use nvisy_core::media::SpreadsheetFormat;
#[cfg(any(feature = "txt", feature = "json", feature = "markdown"))]
use nvisy_core::media::TextFormat;
#[cfg(feature = "docx")]
use nvisy_core::media::WordFormat;

/// Decode [`Content`] into a [`ContentHandle`] using default parameters.
///
/// Dispatches on the content's inferred [`DocumentType`] and routes
/// to the appropriate per-format loader. Returns an error if the
/// document type cannot be inferred or no loader is enabled for it.
pub async fn decode(content: &Content) -> Result<ContentHandle, Error> {
    let doc_type = content.infer_document_type().ok_or_else(|| {
        Error::validation(
            "unable to detect document type from content; \
             set a MIME type via ContentMetadata::with_content_type",
            "nvisy_formats::decode",
        )
    })?;
    let data = content.data();

    match doc_type {
        #[cfg(any(feature = "txt", feature = "json", feature = "markdown", feature = "html"))]
        DocumentType::Text(_) | DocumentType::Html => decode_text(doc_type, data).await,
        #[cfg(any(feature = "csv", feature = "xlsx"))]
        DocumentType::Spreadsheet(_) => decode_tabular(doc_type, data).await,
        #[cfg(any(feature = "png", feature = "jpeg", feature = "tiff"))]
        DocumentType::Image(_) => decode_image(doc_type, data).await,
        #[cfg(any(feature = "wav", feature = "mp3"))]
        DocumentType::Audio(_) => decode_audio(doc_type, data).await,
        #[cfg(any(feature = "pdf", feature = "docx"))]
        DocumentType::Pdf | DocumentType::Word(_) | DocumentType::Presentation(_) => {
            decode_rich(doc_type, data).await
        }
        // Fallback only kicks in when one or more modality
        // feature-sets are off — with every text / tabular / image /
        // audio / rich format enabled, every `DocumentType` variant
        // matches an arm above.
        #[cfg(not(all(
            any(feature = "txt", feature = "json", feature = "markdown", feature = "html"),
            any(feature = "csv", feature = "xlsx"),
            any(feature = "png", feature = "jpeg", feature = "tiff"),
            any(feature = "wav", feature = "mp3"),
            any(feature = "pdf", feature = "docx"),
        )))]
        _ => Err(Error::validation(
            format!("no loader enabled for: {doc_type}"),
            "nvisy_formats::decode",
        )),
    }
}

#[cfg(any(feature = "txt", feature = "json", feature = "markdown", feature = "html"))]
async fn decode_text(doc_type: DocumentType, content: &ContentData) -> Result<ContentHandle, Error> {
    let handler: BoxedTextHandler = match doc_type {
        #[cfg(feature = "txt")]
        DocumentType::Text(TextFormat::Txt | TextFormat::Log) => {
            let h = crate::text::TxtLoader
                .decode(content, &crate::text::TxtParams::default())
                .await?;
            BoxedTextHandler::new(h)
        }
        #[cfg(feature = "json")]
        DocumentType::Text(TextFormat::Json) => {
            let h = crate::text::JsonLoader
                .decode(content, &crate::text::JsonParams::default())
                .await?;
            BoxedTextHandler::new(h)
        }
        #[cfg(feature = "markdown")]
        DocumentType::Text(TextFormat::Markdown) => {
            let h = crate::text::MarkdownLoader
                .decode(content, &crate::text::MarkdownParams::default())
                .await?;
            BoxedTextHandler::new(h)
        }
        #[cfg(feature = "html")]
        DocumentType::Html => {
            let h = crate::text::HtmlLoader
                .decode(content, &crate::text::HtmlParams::default())
                .await?;
            BoxedTextHandler::new(h)
        }
        _ => {
            return Err(Error::validation(
                format!("no text loader for: {doc_type}"),
                "nvisy_formats::decode_text",
            ));
        }
    };
    Ok(ContentHandle::from(handler))
}

#[cfg(any(feature = "csv", feature = "xlsx"))]
async fn decode_tabular(
    doc_type: DocumentType,
    content: &ContentData,
) -> Result<ContentHandle, Error> {
    let handler: BoxedTabularHandler = match doc_type {
        #[cfg(feature = "csv")]
        DocumentType::Spreadsheet(SpreadsheetFormat::Csv) => {
            let h = crate::tabular::CsvLoader
                .decode(content, &crate::tabular::CsvParams::default())
                .await?;
            BoxedTabularHandler::new(h)
        }
        #[cfg(feature = "xlsx")]
        DocumentType::Spreadsheet(SpreadsheetFormat::Xlsx) => {
            let h = crate::tabular::XlsxLoader
                .decode(content, &crate::tabular::XlsxParams)
                .await?;
            BoxedTabularHandler::new(h)
        }
        _ => {
            return Err(Error::validation(
                format!("no tabular loader for: {doc_type}"),
                "nvisy_formats::decode_tabular",
            ));
        }
    };
    Ok(ContentHandle::from(handler))
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "tiff"))]
async fn decode_image(
    doc_type: DocumentType,
    content: &ContentData,
) -> Result<ContentHandle, Error> {
    let handler: BoxedImageHandler = match doc_type {
        #[cfg(feature = "png")]
        DocumentType::Image(ImageFormat::Png) => {
            let h = crate::image::PngLoader
                .decode(content, &crate::image::PngParams)
                .await?;
            BoxedImageHandler::new(h)
        }
        #[cfg(feature = "jpeg")]
        DocumentType::Image(ImageFormat::Jpeg) => {
            let h = crate::image::JpegLoader
                .decode(content, &crate::image::JpegParams)
                .await?;
            BoxedImageHandler::new(h)
        }
        #[cfg(feature = "tiff")]
        DocumentType::Image(ImageFormat::Tiff) => {
            let h = crate::image::TiffLoader
                .decode(content, &crate::image::TiffParams)
                .await?;
            BoxedImageHandler::new(h)
        }
        _ => {
            return Err(Error::validation(
                format!("no image loader for: {doc_type}"),
                "nvisy_formats::decode_image",
            ));
        }
    };
    Ok(ContentHandle::from(handler))
}

#[cfg(any(feature = "wav", feature = "mp3"))]
async fn decode_audio(
    doc_type: DocumentType,
    content: &ContentData,
) -> Result<ContentHandle, Error> {
    let handler: BoxedAudioHandler = match doc_type {
        #[cfg(feature = "wav")]
        DocumentType::Audio(AudioFormat::Wav) => {
            let h = crate::audio::WavLoader
                .decode(content, &crate::audio::WavParams)
                .await?;
            BoxedAudioHandler::new(h)
        }
        #[cfg(feature = "mp3")]
        DocumentType::Audio(AudioFormat::Mp3) => {
            let h = crate::audio::Mp3Loader
                .decode(content, &crate::audio::Mp3Params)
                .await?;
            BoxedAudioHandler::new(h)
        }
        _ => {
            return Err(Error::validation(
                format!("no audio loader for: {doc_type}"),
                "nvisy_formats::decode_audio",
            ));
        }
    };
    Ok(ContentHandle::from(handler))
}

#[cfg(any(feature = "pdf", feature = "docx"))]
async fn decode_rich(
    doc_type: DocumentType,
    content: &ContentData,
) -> Result<ContentHandle, Error> {
    match doc_type {
        #[cfg(feature = "pdf")]
        DocumentType::Pdf => {
            let h = crate::rich::PdfLoader
                .decode(content, &crate::rich::PdfParams::default())
                .await?;
            Ok(ContentHandle::from(BoxedRichHandler::new(h)))
        }
        #[cfg(feature = "docx")]
        DocumentType::Word(WordFormat::Docx) => {
            let h = crate::rich::DocxLoader
                .decode(content, &crate::rich::DocxParams)
                .await?;
            Ok(ContentHandle::from(BoxedRichHandler::new(h)))
        }
        _ => Err(Error::validation(
            format!("no rich loader for: {doc_type}"),
            "nvisy_formats::decode_rich",
        )),
    }
}
