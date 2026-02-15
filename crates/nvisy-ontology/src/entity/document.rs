//! Document format classification.

use serde::{Deserialize, Serialize};

/// Document format that content can be classified as.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "jsonschema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
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
