//! Document format classification.

use std::ffi::OsStr;
use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumString, IntoStaticStr};

/// Image file format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[derive(Display, EnumString, IntoStaticStr, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum ImageFormat {
    Png,
    Jpeg,
    Webp,
    Gif,
    Tiff,
}

impl ImageFormat {
    /// MIME type string for this format.
    pub fn mime_type(self) -> &'static str {
        match self {
            Self::Png => "image/png",
            Self::Jpeg => "image/jpeg",
            Self::Webp => "image/webp",
            Self::Gif => "image/gif",
            Self::Tiff => "image/tiff",
        }
    }

    /// File extension (without leading dot).
    pub fn extension(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpeg => "jpeg",
            Self::Webp => "webp",
            Self::Gif => "gif",
            Self::Tiff => "tiff",
        }
    }

    /// Parse from a MIME type string.
    pub fn from_mime(mime: &str) -> Option<Self> {
        match mime {
            "image/png" => Some(Self::Png),
            "image/jpeg" => Some(Self::Jpeg),
            "image/webp" => Some(Self::Webp),
            "image/gif" => Some(Self::Gif),
            "image/tiff" => Some(Self::Tiff),
            _ => None,
        }
    }
}

/// Word-processor document format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[derive(Display, EnumString, IntoStaticStr, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum WordFormat {
    Doc,
    Docx,
    Odt,
}

impl WordFormat {
    /// MIME type string for this format.
    pub fn mime_type(self) -> &'static str {
        match self {
            Self::Doc => "application/msword",
            Self::Docx => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
            Self::Odt => "application/vnd.oasis.opendocument.text",
        }
    }

    /// Parse from a MIME type string.
    pub fn from_mime(mime: &str) -> Option<Self> {
        match mime {
            "application/msword" => Some(Self::Doc),
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document" => {
                Some(Self::Docx)
            }
            "application/vnd.oasis.opendocument.text" => Some(Self::Odt),
            _ => None,
        }
    }
}

/// Presentation document format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[derive(Display, EnumString, IntoStaticStr, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum PresentationFormat {
    Ppt,
    Pptx,
    Odp,
}

impl PresentationFormat {
    /// MIME type string for this format.
    pub fn mime_type(self) -> &'static str {
        match self {
            Self::Ppt => "application/vnd.ms-powerpoint",
            Self::Pptx => {
                "application/vnd.openxmlformats-officedocument.presentationml.presentation"
            }
            Self::Odp => "application/vnd.oasis.opendocument.presentation",
        }
    }

    /// Parse from a MIME type string.
    pub fn from_mime(mime: &str) -> Option<Self> {
        match mime {
            "application/vnd.ms-powerpoint" => Some(Self::Ppt),
            "application/vnd.openxmlformats-officedocument.presentationml.presentation" => {
                Some(Self::Pptx)
            }
            "application/vnd.oasis.opendocument.presentation" => Some(Self::Odp),
            _ => None,
        }
    }
}

/// Spreadsheet document format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[derive(Display, EnumString, IntoStaticStr, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum SpreadsheetFormat {
    Xls,
    Xlsx,
    Xlsm,
    Xltx,
    Csv,
    Ods,
}

impl SpreadsheetFormat {
    /// MIME type string for this format.
    pub fn mime_type(self) -> &'static str {
        match self {
            Self::Xls => "application/vnd.ms-excel",
            Self::Xlsx => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
            Self::Xlsm => "application/vnd.ms-excel.sheet.macroEnabled.12",
            Self::Xltx => "application/vnd.openxmlformats-officedocument.spreadsheetml.template",
            Self::Csv => "text/csv",
            Self::Ods => "application/vnd.oasis.opendocument.spreadsheet",
        }
    }

    /// Parse from a MIME type string.
    pub fn from_mime(mime: &str) -> Option<Self> {
        match mime {
            "application/vnd.ms-excel" => Some(Self::Xls),
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet" => Some(Self::Xlsx),
            "application/vnd.ms-excel.sheet.macroEnabled.12" => Some(Self::Xlsm),
            "application/vnd.openxmlformats-officedocument.spreadsheetml.template" => {
                Some(Self::Xltx)
            }
            "text/csv" => Some(Self::Csv),
            "application/vnd.oasis.opendocument.spreadsheet" => Some(Self::Ods),
            _ => None,
        }
    }
}

/// Audio file format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[derive(Display, EnumString, IntoStaticStr, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum AudioFormat {
    Wav,
    Mp3,
}

impl AudioFormat {
    /// MIME type string for this format.
    pub fn mime_type(self) -> &'static str {
        match self {
            Self::Wav => "audio/wav",
            Self::Mp3 => "audio/mpeg",
        }
    }

    /// Parse from a MIME type string.
    pub fn from_mime(mime: &str) -> Option<Self> {
        match mime {
            "audio/wav" | "audio/x-wav" => Some(Self::Wav),
            "audio/mpeg" => Some(Self::Mp3),
            _ => None,
        }
    }
}

/// Plain text format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[derive(Display, EnumString, IntoStaticStr, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum TextFormat {
    Txt,
    Log,
    Json,
    Markdown,
}

impl TextFormat {
    /// MIME type string for this format.
    pub fn mime_type(self) -> &'static str {
        match self {
            Self::Txt | Self::Log => "text/plain",
            Self::Json => "application/json",
            Self::Markdown => "text/markdown",
        }
    }

    /// Parse from a MIME type string.
    pub fn from_mime(mime: &str) -> Option<Self> {
        match mime {
            "text/plain" => Some(Self::Txt),
            "application/json" => Some(Self::Json),
            "text/markdown" => Some(Self::Markdown),
            _ => None,
        }
    }
}

/// Document format that content can be classified as.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DocumentType {
    Text(TextFormat),
    Image(ImageFormat),
    Word(WordFormat),
    Presentation(PresentationFormat),
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
            Self::Presentation(fmt_) => write!(f, "{fmt_}"),
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
        None.or_else(|| TextFormat::from_mime(mime).map(Self::Text))
            .or_else(|| ImageFormat::from_mime(mime).map(Self::Image))
            .or_else(|| WordFormat::from_mime(mime).map(Self::Word))
            .or_else(|| PresentationFormat::from_mime(mime).map(Self::Presentation))
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
            "ppt" => Some(Self::Presentation(PresentationFormat::Ppt)),
            "pptx" => Some(Self::Presentation(PresentationFormat::Pptx)),
            "odp" => Some(Self::Presentation(PresentationFormat::Odp)),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsStr;

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
