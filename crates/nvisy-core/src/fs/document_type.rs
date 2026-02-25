//! Document format classification.

use serde::{Deserialize, Serialize};
use strum::{Display, EnumString};

/// Document format that content can be classified as.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Display, EnumString, Serialize, Deserialize, schemars::JsonSchema)]
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
