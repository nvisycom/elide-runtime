//! Small per-modality format tags.
//!
//! [`ImageFormat`] / [`AudioFormat`] are useful when a piece of code
//! needs to know "what kind of bytes is this" without going through a
//! full codec handler — for example, the OCR backend's
//! [`ImageInput`][ii] attaches one of these to the raw bytes it sends
//! over the wire so the backend knows how to decode.
//!
//! [ii]: nvisy_ocr::ImageInput

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
