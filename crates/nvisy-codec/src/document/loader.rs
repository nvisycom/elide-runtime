//! [`UniversalLoader`]: auto-detect format and dispatch to the
//! appropriate typed loader.

use nvisy_core::Error;
use nvisy_core::fs::DocumentType;
use nvisy_core::io::ContentData;

use crate::document::AnyDocument;
use crate::handler::Loader;

/// Format-agnostic loader that detects the document type from MIME
/// type and magic bytes, then delegates to the appropriate typed
/// loader with default parameters.
pub struct UniversalLoader;

impl UniversalLoader {
    /// Detect format and decode the content into an [`AnyDocument`].
    ///
    /// Detection priority:
    /// 1. `content.content_type()` — caller-provided MIME
    /// 2. `infer::get()` — magic-byte detection
    /// 3. Heuristic: UTF-8 → JSON parse → CSV sniff → fallback to Txt
    pub async fn decode(
        &self,
        content: &ContentData,
    ) -> Result<AnyDocument, Error> {
        let doc_type = detect_type(content);
        tracing::debug!(?doc_type, "universal loader detected format");
        dispatch(doc_type, content).await
    }
}

/// Detect the document type from the content's MIME type, magic bytes,
/// and heuristic content analysis.
fn detect_type(content: &ContentData) -> DocumentType {
    // 1. Caller-provided MIME
    if let Some(mime) = content.content_type()
        && let Some(dt) = DocumentType::from_mime(mime)
    {
        return dt;
    }

    // 2. Magic-byte detection via `infer`
    let bytes = content.as_bytes();
    if let Some(kind) = infer::get(bytes)
        && let Some(dt) = DocumentType::from_mime(kind.mime_type())
    {
        return dt;
    }

    // 3. Heuristic: try UTF-8, then probe content
    let Ok(text) = std::str::from_utf8(bytes) else {
        // Not valid UTF-8 — treat as plain text (handler will store
        // raw bytes anyway).
        return DocumentType::Txt;
    };

    let trimmed = text.trim();

    // JSON: starts with `{` or `[`
    if (trimmed.starts_with('{') || trimmed.starts_with('['))
        && serde_json::from_str::<serde_json::Value>(trimmed).is_ok()
    {
        return DocumentType::Json;
    }

    // CSV: sniff for delimiters in the first line
    if let Some(first_line) = trimmed.lines().next() {
        let has_comma = first_line.contains(',');
        let has_tab = first_line.contains('\t');
        let has_semicolon = first_line.contains(';');
        let has_pipe = first_line.contains('|');
        if (has_comma || has_tab || has_semicolon || has_pipe) && trimmed.lines().count() > 1 {
            return DocumentType::Csv;
        }
    }

    DocumentType::Txt
}

/// Dispatch to the appropriate typed loader with default parameters.
async fn dispatch(doc_type: DocumentType, content: &ContentData) -> Result<AnyDocument, Error> {
    match doc_type {
        DocumentType::Txt => {
            let doc = crate::handler::TxtLoader
                .decode(content, &Default::default())
                .await?;
            Ok(AnyDocument::Txt(doc))
        }
        DocumentType::Csv => {
            let doc = crate::handler::CsvLoader
                .decode(content, &Default::default())
                .await?;
            Ok(AnyDocument::Csv(doc))
        }
        DocumentType::Json => {
            let doc = crate::handler::JsonLoader
                .decode(content, &Default::default())
                .await?;
            Ok(AnyDocument::Json(doc))
        }
        DocumentType::Html => {
            #[cfg(feature = "html")]
            {
                let doc = crate::handler::HtmlLoader
                    .decode(content, &Default::default())
                    .await?;
                Ok(AnyDocument::Html(doc))
            }
            #[cfg(not(feature = "html"))]
            Err(Error::validation(
                "HTML support requires the `html` feature",
                "universal-loader",
            ))
        }
        DocumentType::Png => {
            let doc = crate::handler::PngLoader
                .decode(content, &Default::default())
                .await?;
            Ok(AnyDocument::Image(doc.map_handler(Into::into)))
        }
        DocumentType::Jpeg => {
            let doc = crate::handler::JpegLoader
                .decode(content, &Default::default())
                .await?;
            Ok(AnyDocument::Image(doc.map_handler(Into::into)))
        }
        DocumentType::Wav => {
            let doc = crate::handler::WavLoader
                .decode(content, &Default::default())
                .await?;
            Ok(AnyDocument::Audio(doc.map_handler(Into::into)))
        }
        DocumentType::Mp3 => {
            let doc = crate::handler::Mp3Loader
                .decode(content, &Default::default())
                .await?;
            Ok(AnyDocument::Audio(doc.map_handler(Into::into)))
        }
        DocumentType::Pdf => {
            #[cfg(feature = "pdf")]
            {
                let doc = crate::handler::PdfLoader
                    .decode(content, &Default::default())
                    .await?;
                Ok(AnyDocument::Pdf(doc))
            }
            #[cfg(not(feature = "pdf"))]
            Err(Error::validation(
                "PDF support requires the `pdf` feature",
                "universal-loader",
            ))
        }
        DocumentType::Docx => {
            #[cfg(feature = "docx")]
            {
                let doc = crate::handler::DocxLoader
                    .decode(content, &Default::default())
                    .await?;
                Ok(AnyDocument::Docx(doc))
            }
            #[cfg(not(feature = "docx"))]
            Err(Error::validation(
                "DOCX support requires the `docx` feature",
                "universal-loader",
            ))
        }
        DocumentType::Xlsx => {
            #[cfg(feature = "xlsx")]
            {
                let doc = crate::handler::XlsxLoader
                    .decode(content, &Default::default())
                    .await?;
                Ok(AnyDocument::Xlsx(doc))
            }
            #[cfg(not(feature = "xlsx"))]
            Err(Error::validation(
                "XLSX support requires the `xlsx` feature",
                "universal-loader",
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;
    use nvisy_core::path::ContentSource;

    fn content_with_mime(data: &[u8], mime: &str) -> ContentData {
        let mut c = ContentData::new(ContentSource::new(), Bytes::copy_from_slice(data));
        c.mime = Some(mime.to_string());
        c
    }

    fn content_raw(data: &[u8]) -> ContentData {
        ContentData::new(ContentSource::new(), Bytes::copy_from_slice(data))
    }

    #[tokio::test]
    async fn json_string_with_mime() {
        let content = content_with_mime(b"{\"key\": \"value\"}", "application/json");
        let doc = UniversalLoader.decode(&content).await.unwrap();
        assert_eq!(doc.document_type(), DocumentType::Json);
    }

    #[tokio::test]
    async fn wav_bytes_detected() {
        // Minimal WAV header: RIFF....WAVEfmt
        let mut wav = Vec::new();
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&36u32.to_le_bytes()); // chunk size
        wav.extend_from_slice(b"WAVE");
        wav.extend_from_slice(b"fmt ");
        wav.extend_from_slice(&16u32.to_le_bytes()); // subchunk size
        wav.extend_from_slice(&1u16.to_le_bytes()); // PCM
        wav.extend_from_slice(&1u16.to_le_bytes()); // mono
        wav.extend_from_slice(&44100u32.to_le_bytes()); // sample rate
        wav.extend_from_slice(&44100u32.to_le_bytes()); // byte rate
        wav.extend_from_slice(&1u16.to_le_bytes()); // block align
        wav.extend_from_slice(&8u16.to_le_bytes()); // bits per sample
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&0u32.to_le_bytes()); // data size

        let content = content_raw(&wav);
        let doc = UniversalLoader.decode(&content).await.unwrap();
        assert_eq!(doc.document_type(), DocumentType::Wav);
    }

    #[tokio::test]
    async fn unknown_bytes_fallback_to_txt() {
        let content = content_raw(b"just some plain text\nwith lines");
        let doc = UniversalLoader.decode(&content).await.unwrap();
        assert_eq!(doc.document_type(), DocumentType::Txt);
    }

    #[tokio::test]
    async fn json_heuristic_detection() {
        let content = content_raw(b"{\"hello\": \"world\"}");
        let doc = UniversalLoader.decode(&content).await.unwrap();
        assert_eq!(doc.document_type(), DocumentType::Json);
    }

    #[tokio::test]
    async fn csv_heuristic_detection() {
        let content = content_raw(b"name,age\nAlice,30\nBob,25\n");
        let doc = UniversalLoader.decode(&content).await.unwrap();
        assert_eq!(doc.document_type(), DocumentType::Csv);
    }

    #[tokio::test]
    async fn png_mime_detection() {
        // Valid 1x1 white PNG generated with correct CRCs
        let png_bytes: &[u8] = &[
            0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A,
            0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52,
            0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01,
            0x08, 0x02, 0x00, 0x00, 0x00, 0x90, 0x77, 0x53,
            0xDE, 0x00, 0x00, 0x00, 0x0C, 0x49, 0x44, 0x41,
            0x54, 0x78, 0x9C, 0x63, 0xF8, 0xFF, 0xFF, 0x3F,
            0x00, 0x05, 0xFE, 0x02, 0xFE, 0x0D, 0xEF, 0x46,
            0xB8, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E,
            0x44, 0xAE, 0x42, 0x60, 0x82,
        ];
        let content = content_raw(png_bytes);
        let doc = UniversalLoader.decode(&content).await.unwrap();
        assert_eq!(doc.document_type(), DocumentType::Png);
    }

    #[tokio::test]
    async fn jpeg_mime_detection() {
        let content = content_with_mime(b"\xff\xd8\xff\xe0jfif-data", "image/jpeg");
        let doc = UniversalLoader.decode(&content).await;
        // JPEG decoding may fail on minimal bytes, but the type detection should work
        // If it fails, it's a decode error not a detection error
        match doc {
            Ok(d) => assert_eq!(d.document_type(), DocumentType::Jpeg),
            Err(e) => assert!(e.to_string().contains("decode"), "unexpected error: {e}"),
        }
    }
}
