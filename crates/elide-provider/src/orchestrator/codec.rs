//! [`CodecParams`]: how a document's bytes are turned into content.
//!
//! Separate from [`DocumentContext`] because the two feed different
//! subsystems: this drives the codec, that drives recognition.
//! Recorded on the audit all the same, because anonymize must decode
//! the document exactly as analyze did — entity offsets are stored
//! against the first decode, and a differently-rendered second one
//! would not line up.
//!
//! [`DocumentContext`]: super::DocumentContext

use elide::primitive::RasterMode;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// How the codec decodes this document.
///
/// Defaults to the codec's own behaviour, so a caller with no
/// opinion passes [`CodecParams::default`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct CodecParams {
    /// How container formats carrying both a text layer and page
    /// images treat OCR.
    ///
    /// Defaults to [`RasterMode::Auto`], the codec's own behaviour.
    pub raster_mode: RasterMode,
}

impl CodecParams {
    /// Params leaving every codec at its default.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The same params, decoding under `mode`.
    #[must_use]
    pub fn with_raster_mode(mut self, mode: RasterMode) -> Self {
        self.raster_mode = mode;
        self
    }
}
