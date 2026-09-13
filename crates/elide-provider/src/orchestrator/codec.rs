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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct CodecParams {
    /// How container formats carrying both a text layer and page
    /// images treat OCR.
    ///
    /// Defaults to [`RasterMode::Auto`], the codec's own behaviour.
    pub raster_mode: RasterMode,
    /// Whether a CSV's first row is its header.
    ///
    /// Defaults to `true`, the codec's own behaviour. A header row
    /// gains column-name context hints for the data below it, but
    /// is protected from a row-drop redaction — so a *headerless*
    /// file needs `false`, or its first row of real data cannot be
    /// dropped.
    pub csv_has_headers: bool,
    /// The CSV field separator, or [`None`] to auto-detect it.
    ///
    /// Defaults to [`None`], the codec's own behaviour. Detection
    /// falls back to a comma when nothing stands out, which can
    /// misread a TSV or semicolon-delimited file, so pass the byte
    /// when the format is known.
    pub csv_delimiter: Option<u8>,
    /// What happens to an image's EXIF metadata on re-encode.
    ///
    /// Defaults to [`ExifMetadata::Keep`], the codec's own
    /// behaviour.
    ///
    /// This governs the output only when no metadata pipeline ran:
    /// a wired EXIF recognizer and anonymizer strip through the
    /// `#exif` sub-part and always win. It is the knob for
    /// stripping unconditionally *without* wiring one.
    pub exif_metadata: ExifMetadata,
}

/// What happens to an image's EXIF metadata when it is re-encoded.
///
/// Mirrors the codec's own `ExifPolicy`, declared here instead of
/// re-exported so that [`CodecParams`] — a wire type every request
/// carries — does not depend on the image codec features. The
/// decode path translates it where those features live.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ExifMetadata {
    /// Leave the metadata untouched. The codec's registered
    /// default, and so this crate's.
    #[default]
    Keep,
    /// Drop only the privacy-sensitive fields (GPS, device,
    /// timestamps), keeping what a viewer needs to render the
    /// image as intended.
    StripSensitive,
    /// Drop the entire metadata block, benign fields included.
    StripAll,
}

impl Default for CodecParams {
    /// Every codec at the behaviour it has when nothing is
    /// configured.
    ///
    /// Written out rather than derived: `csv_has_headers` defaults
    /// to `true` (the codec treats a first row as a header), which
    /// `bool`'s own `Default` of `false` would invert — silently
    /// turning every CSV's header into a droppable data row.
    fn default() -> Self {
        Self {
            raster_mode: RasterMode::default(),
            csv_has_headers: true,
            csv_delimiter: None,
            exif_metadata: ExifMetadata::Keep,
        }
    }
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

    /// The same params, reading CSV with or without a header row.
    #[must_use]
    pub fn with_csv_has_headers(mut self, has_headers: bool) -> Self {
        self.csv_has_headers = has_headers;
        self
    }

    /// The same params, reading CSV with an explicit `delimiter`.
    #[must_use]
    pub fn with_csv_delimiter(mut self, delimiter: u8) -> Self {
        self.csv_delimiter = Some(delimiter);
        self
    }

    /// The same params, re-encoding images under `metadata`.
    #[must_use]
    pub fn with_exif_metadata(mut self, metadata: ExifMetadata) -> Self {
        self.exif_metadata = metadata;
        self
    }

    /// Whether these params leave every codec at its default.
    ///
    /// The decode path uses this to decide whether a request can
    /// share the prebuilt registry or has to rebuild one.
    #[must_use]
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}
