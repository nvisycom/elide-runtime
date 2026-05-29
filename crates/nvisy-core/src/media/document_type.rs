//! [`DocumentType`]: top-level document classification that nests
//! the per-category leaf format enums from
//! [`document_format`](super::document_format).

use std::ffi::OsStr;
use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::document_format::{AudioFormat, ImageFormat, SpreadsheetFormat, TextFormat, WordFormat};

/// Document format that content can be classified as.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DocumentType {
    Text(TextFormat),
    Image(ImageFormat),
    Word(WordFormat),
    Spreadsheet(SpreadsheetFormat),
    Audio(AudioFormat),
    Html,
    Pdf,
}

impl fmt::Display for DocumentType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Text(fmt_) => write!(f, "{fmt_}"),
            Self::Image(fmt_) => write!(f, "{fmt_}"),
            Self::Word(fmt_) => write!(f, "{fmt_}"),
            Self::Spreadsheet(fmt_) => write!(f, "{fmt_}"),
            Self::Audio(fmt_) => write!(f, "{fmt_}"),
            Self::Html => write!(f, "html"),
            Self::Pdf => write!(f, "pdf"),
        }
    }
}

impl DocumentType {
    /// Map a MIME type string to a [`DocumentType`].
    ///
    /// Returns `None` for unrecognised MIME types.
    pub fn from_mime(mime: &str) -> Option<Self> {
        TextFormat::from_mime(mime)
            .map(Self::Text)
            .or_else(|| ImageFormat::from_mime(mime).map(Self::Image))
            .or_else(|| WordFormat::from_mime(mime).map(Self::Word))
            .or_else(|| SpreadsheetFormat::from_mime(mime).map(Self::Spreadsheet))
            .or_else(|| AudioFormat::from_mime(mime).map(Self::Audio))
            .or(match mime {
                "text/html" => Some(Self::Html),
                "application/pdf" => Some(Self::Pdf),
                _ => None,
            })
    }

    /// Map a file extension (without leading dot) to a [`DocumentType`].
    ///
    /// Handles cases where the MIME type alone is ambiguous (e.g.
    /// `text/plain` cannot distinguish `.txt` from `.log`).
    pub fn from_extension(ext: &OsStr) -> Option<Self> {
        let ext = ext.to_str()?;
        match ext.to_ascii_lowercase().as_str() {
            "txt" => Some(Self::Text(TextFormat::Txt)),
            "log" => Some(Self::Text(TextFormat::Log)),
            "json" => Some(Self::Text(TextFormat::Json)),
            "md" | "markdown" => Some(Self::Text(TextFormat::Markdown)),
            "csv" => Some(Self::Spreadsheet(SpreadsheetFormat::Csv)),
            "html" | "htm" => Some(Self::Html),
            "png" => Some(Self::Image(ImageFormat::Png)),
            "jpg" | "jpeg" => Some(Self::Image(ImageFormat::Jpeg)),
            "webp" => Some(Self::Image(ImageFormat::Webp)),
            "gif" => Some(Self::Image(ImageFormat::Gif)),
            "tiff" | "tif" => Some(Self::Image(ImageFormat::Tiff)),
            "wav" => Some(Self::Audio(AudioFormat::Wav)),
            "mp3" => Some(Self::Audio(AudioFormat::Mp3)),
            "pdf" => Some(Self::Pdf),
            "doc" => Some(Self::Word(WordFormat::Doc)),
            "docx" => Some(Self::Word(WordFormat::Docx)),
            "odt" => Some(Self::Word(WordFormat::Odt)),
            "xls" => Some(Self::Spreadsheet(SpreadsheetFormat::Xls)),
            "xlsx" => Some(Self::Spreadsheet(SpreadsheetFormat::Xlsx)),
            "xlsm" => Some(Self::Spreadsheet(SpreadsheetFormat::Xlsm)),
            "xltx" => Some(Self::Spreadsheet(SpreadsheetFormat::Xltx)),
            "ods" => Some(Self::Spreadsheet(SpreadsheetFormat::Ods)),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_mime_unknown_returns_none() {
        assert_eq!(DocumentType::from_mime("application/octet-stream"), None);
        assert_eq!(DocumentType::from_mime("video/mp4"), None);
        assert_eq!(DocumentType::from_mime(""), None);
    }

    #[test]
    fn from_mime_alias() {
        assert_eq!(
            DocumentType::from_mime("audio/x-wav"),
            Some(DocumentType::Audio(AudioFormat::Wav)),
        );
    }

    #[test]
    fn from_extension_common_formats() {
        assert_eq!(
            DocumentType::from_extension(OsStr::new("png")),
            Some(DocumentType::Image(ImageFormat::Png)),
        );
        assert_eq!(
            DocumentType::from_extension(OsStr::new("log")),
            Some(DocumentType::Text(TextFormat::Log)),
        );
        assert_eq!(
            DocumentType::from_extension(OsStr::new("PDF")),
            Some(DocumentType::Pdf),
        );
        assert_eq!(DocumentType::from_extension(OsStr::new("unknown")), None);
    }
}
