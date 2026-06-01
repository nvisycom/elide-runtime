//! Per-category document format enums (`ImageFormat`, `WordFormat`,
//! `SpreadsheetFormat`, `AudioFormat`, `TextFormat`).
//!
//! These are the leaf sub-format types nested inside
//! [`DocumentType`]. Each enum knows how to
//! parse itself from a MIME type via `from_mime`. Only
//! `ImageFormat` carries a `mime_type` accessor — it's the one
//! variant the workspace needs to emit a MIME string for
//! (rendering boundary in `nvisy-ocr`).
//!
//! [`DocumentType`]: super::DocumentType

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
