//! [`decode`]: dispatch a [`Content`] to the appropriate loader by
//! its detected document type and return a [`DocumentHandle`].

use nvisy_codec::DocumentHandle;
#[cfg(any(
    feature = "internal_text",
    feature = "internal_tabular",
    feature = "internal_image",
    feature = "internal_audio",
    feature = "internal_rich",
))]
use nvisy_codec::handler::{Handle, Loader};
#[cfg(feature = "internal_rich")]
use nvisy_codec::handler::RichHandle;
#[cfg(feature = "internal_audio")]
use nvisy_ontology::modality::Audio;
#[cfg(feature = "internal_image")]
use nvisy_ontology::modality::Image;
#[cfg(feature = "internal_tabular")]
use nvisy_ontology::modality::Tabular;
#[cfg(feature = "internal_text")]
use nvisy_ontology::modality::Text;
use nvisy_core::Error;
use nvisy_core::content::Content;
#[cfg(any(
    feature = "internal_text",
    feature = "internal_tabular",
    feature = "internal_image",
    feature = "internal_audio",
    feature = "internal_rich",
))]
use nvisy_core::content::ContentData;
#[cfg(feature = "internal_audio")]
use nvisy_core::media::AudioFormat;
#[cfg(any(
    feature = "internal_text",
    feature = "internal_tabular",
    feature = "internal_image",
    feature = "internal_audio",
    feature = "internal_rich",
))]
use nvisy_core::media::DocumentType;
#[cfg(feature = "internal_image")]
use nvisy_core::media::ImageFormat;
#[cfg(feature = "internal_tabular")]
use nvisy_core::media::SpreadsheetFormat;
#[cfg(feature = "internal_text")]
use nvisy_core::media::TextFormat;
#[cfg(feature = "docx")]
use nvisy_core::media::WordFormat;

/// Decode [`Content`] into a [`DocumentHandle`] using default parameters.
///
/// Dispatches on the content's inferred [`DocumentType`] and routes
/// to the appropriate per-format loader. Returns an error if the
/// document type cannot be inferred or no loader is enabled for it.
pub async fn decode(content: &Content) -> Result<DocumentHandle, Error> {
    let doc_type = content.infer_document_type().ok_or_else(|| {
        Error::validation(
            "unable to detect document type from content; \
             set a MIME type via ContentMetadata::with_content_type",
            "nvisy_formats::decode",
        )
    })?;
    let _data = content.data();

    #[cfg(feature = "internal_text")]
    if let Some(h) = try_decode_text(doc_type, _data).await? {
        return Ok(DocumentHandle::Text(h));
    }
    #[cfg(feature = "internal_tabular")]
    if let Some(h) = try_decode_tabular(doc_type, _data).await? {
        return Ok(DocumentHandle::Tabular(h));
    }
    #[cfg(feature = "internal_image")]
    if let Some(h) = try_decode_image(doc_type, _data).await? {
        return Ok(DocumentHandle::Image(h));
    }
    #[cfg(feature = "internal_audio")]
    if let Some(h) = try_decode_audio(doc_type, _data).await? {
        return Ok(DocumentHandle::Audio(h));
    }
    #[cfg(feature = "internal_rich")]
    if let Some(h) = try_decode_rich(doc_type, _data).await? {
        return Ok(DocumentHandle::Rich(h));
    }

    Err(Error::validation(
        format!("no loader enabled for: {doc_type}"),
        "nvisy_formats::decode",
    ))
}

#[cfg(feature = "internal_text")]
async fn try_decode_text(
    doc_type: DocumentType,
    content: &ContentData,
) -> Result<Option<Box<dyn Handle<Text>>>, Error> {
    let handler: Box<dyn Handle<Text>> = match doc_type {
        #[cfg(feature = "txt")]
        DocumentType::Text(TextFormat::Txt | TextFormat::Log) => {
            let h = crate::text::TxtLoader
                .decode(content, &crate::text::TxtParams::default())
                .await?;
            Box::new(h)
        }
        #[cfg(feature = "json")]
        DocumentType::Text(TextFormat::Json) => {
            let h = crate::text::JsonLoader
                .decode(content, &crate::text::JsonParams::default())
                .await?;
            Box::new(h)
        }
        #[cfg(feature = "markdown")]
        DocumentType::Text(TextFormat::Markdown) => {
            let h = crate::text::MarkdownLoader
                .decode(content, &crate::text::MarkdownParams::default())
                .await?;
            Box::new(h)
        }
        #[cfg(feature = "html")]
        DocumentType::Html => {
            let h = crate::text::HtmlLoader
                .decode(content, &crate::text::HtmlParams::default())
                .await?;
            Box::new(h)
        }
        _ => return Ok(None),
    };
    Ok(Some(handler))
}

#[cfg(feature = "internal_tabular")]
async fn try_decode_tabular(
    doc_type: DocumentType,
    content: &ContentData,
) -> Result<Option<Box<dyn Handle<Tabular>>>, Error> {
    let handler: Box<dyn Handle<Tabular>> = match doc_type {
        #[cfg(feature = "csv")]
        DocumentType::Spreadsheet(SpreadsheetFormat::Csv) => {
            let h = crate::tabular::CsvLoader
                .decode(content, &crate::tabular::CsvParams::default())
                .await?;
            Box::new(h)
        }
        #[cfg(feature = "xlsx")]
        DocumentType::Spreadsheet(SpreadsheetFormat::Xlsx) => {
            let h = crate::tabular::XlsxLoader
                .decode(content, &crate::tabular::XlsxParams)
                .await?;
            Box::new(h)
        }
        _ => return Ok(None),
    };
    Ok(Some(handler))
}

#[cfg(feature = "internal_image")]
async fn try_decode_image(
    doc_type: DocumentType,
    content: &ContentData,
) -> Result<Option<Box<dyn Handle<Image>>>, Error> {
    let handler: Box<dyn Handle<Image>> = match doc_type {
        #[cfg(feature = "png")]
        DocumentType::Image(ImageFormat::Png) => {
            let h = crate::image::PngLoader
                .decode(content, &crate::image::PngParams)
                .await?;
            Box::new(h)
        }
        #[cfg(feature = "jpeg")]
        DocumentType::Image(ImageFormat::Jpeg) => {
            let h = crate::image::JpegLoader
                .decode(content, &crate::image::JpegParams)
                .await?;
            Box::new(h)
        }
        #[cfg(feature = "tiff")]
        DocumentType::Image(ImageFormat::Tiff) => {
            let h = crate::image::TiffLoader
                .decode(content, &crate::image::TiffParams)
                .await?;
            Box::new(h)
        }
        _ => return Ok(None),
    };
    Ok(Some(handler))
}

#[cfg(feature = "internal_audio")]
async fn try_decode_audio(
    doc_type: DocumentType,
    content: &ContentData,
) -> Result<Option<Box<dyn Handle<Audio>>>, Error> {
    let handler: Box<dyn Handle<Audio>> = match doc_type {
        #[cfg(feature = "wav")]
        DocumentType::Audio(AudioFormat::Wav) => {
            let h = crate::audio::WavLoader
                .decode(content, &crate::audio::WavParams)
                .await?;
            Box::new(h)
        }
        #[cfg(feature = "mp3")]
        DocumentType::Audio(AudioFormat::Mp3) => {
            let h = crate::audio::Mp3Loader
                .decode(content, &crate::audio::Mp3Params)
                .await?;
            Box::new(h)
        }
        _ => return Ok(None),
    };
    Ok(Some(handler))
}

#[cfg(feature = "internal_rich")]
async fn try_decode_rich(
    doc_type: DocumentType,
    content: &ContentData,
) -> Result<Option<Box<dyn RichHandle>>, Error> {
    let handler: Box<dyn RichHandle> = match doc_type {
        #[cfg(feature = "pdf")]
        DocumentType::Pdf => {
            let h = crate::rich::PdfLoader
                .decode(content, &crate::rich::PdfParams::default())
                .await?;
            Box::new(h)
        }
        #[cfg(feature = "docx")]
        DocumentType::Word(WordFormat::Docx) => {
            let h = crate::rich::DocxLoader
                .decode(content, &crate::rich::DocxParams)
                .await?;
            Box::new(h)
        }
        _ => return Ok(None),
    };
    Ok(Some(handler))
}
