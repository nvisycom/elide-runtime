//! Document format classification.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumString};

/// Document format that content can be classified as.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Display, EnumString, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum DocumentType {
    /// Plain text (`.txt`, `.log`, etc.).
    Txt,
    /// Comma-separated values.
    Csv,
    /// JSON data.
    Json,
    /// HTML pages.
    Html,
    /// PDF documents.
    Pdf,
    /// Microsoft Word (`.docx`).
    Docx,
    /// Microsoft Excel (`.xlsx`).
    Xlsx,
    /// PNG image.
    Png,
    /// JPEG image.
    Jpeg,
    /// WAV audio.
    Wav,
    /// MP3 audio.
    Mp3,
}

impl DocumentType {
    /// Map a MIME type string to a [`DocumentType`].
    ///
    /// Returns `None` for unrecognised MIME types.
    pub fn from_mime(mime: &str) -> Option<Self> {
        match mime {
            "text/plain" => Some(Self::Txt),
            "text/csv" => Some(Self::Csv),
            "application/json" => Some(Self::Json),
            "text/html" => Some(Self::Html),
            "image/png" => Some(Self::Png),
            "image/jpeg" => Some(Self::Jpeg),
            "audio/x-wav" | "audio/wav" => Some(Self::Wav),
            "audio/mpeg" => Some(Self::Mp3),
            "application/pdf" => Some(Self::Pdf),
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document" => {
                Some(Self::Docx)
            }
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet" => {
                Some(Self::Xlsx)
            }
            _ => None,
        }
    }
}
