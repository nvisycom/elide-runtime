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

use elide::redaction::HiddenContentPolicy;
use elide_pdf::primitive::RasterMode;
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
    /// What decode does with the document's metadata and its
    /// collaboration content (comments, tracked changes, annotations).
    ///
    /// Defaults to [`KeepOnlyEssential`] for both: stripped unexamined,
    /// which is what the report's `stripped` list records.
    /// [`ScanAsContent`] keeps the content and sends it through
    /// detection, so whatever detection does not find stays in the
    /// output. [`FlagEverything`] keeps it too, and redacts every item
    /// the format labels whatever the catalog declares.
    ///
    /// Metadata a format holds as text, such as HTML's `<meta>` fields,
    /// goes through text detection. Metadata it surfaces field by field,
    /// such as EXIF or office document properties, goes through elide's
    /// built-in metadata pipeline: a field is selected by the label its
    /// format gives it, and erased when the request's catalog declares
    /// that label. The policy set's rules do not reach those fields.
    ///
    /// [`KeepOnlyEssential`]: elide::redaction::Disposition::KeepOnlyEssential
    /// [`ScanAsContent`]: elide::redaction::Disposition::ScanAsContent
    /// [`FlagEverything`]: elide::redaction::Disposition::FlagEverything
    pub hidden_content: HiddenContentPolicy,
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
            hidden_content: HiddenContentPolicy::default(),
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

    /// The same params, handling hidden content under `policy`.
    #[must_use]
    pub fn with_hidden_content(mut self, policy: HiddenContentPolicy) -> Self {
        self.hidden_content = policy;
        self
    }

    /// Whether these params leave every registered format at its
    /// default.
    ///
    /// The decode path uses this to decide whether a request can
    /// share the prebuilt registry or has to rebuild one. The
    /// hidden-content policy plays no part: it is an argument to each
    /// decode, not a setting on a format.
    #[must_use]
    pub fn uses_default_formats(&self) -> bool {
        let default = Self::default();
        self.raster_mode == default.raster_mode
            && self.csv_has_headers == default.csv_has_headers
            && self.csv_delimiter == default.csv_delimiter
    }
}
