//! Per-modality [`Modality::Extraction`] enums recorded on each
//! [`Modality::Metadata`], naming how a document's primary content was
//! produced.
//!
//! Extraction is an importer-time property of the [`Document<M>`], not
//! of the entities inside it. A PDF with a text layer is
//! [`TextExtraction::Native`]; the same PDF page rendered and OCR'd is
//! [`TextExtraction::Recognized`]; an image-modality envelope produced
//! by an OCR pipeline is [`ImageExtraction::Ocr`]. Recognition (how an
//! entity was found in the extracted content) is a separate concern
//! tracked on [`RecognitionMethod`].
//!
//! Container-format metadata extraction (EXIF, ID3, PDF `/Info`,
//! DOCX core properties) is tracked separately — see issue #230.
//!
//! [`Modality::Extraction`]: super::Modality::Extraction
//! [`Modality::Metadata`]: super::Modality::Metadata
//! [`Document<M>`]: crate::document::Document
//! [`RecognitionMethod`]: crate::entity::RecognitionMethod

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::entity::ModelProvenance;

/// How a [`Document<Text>`]'s text content was produced.
///
/// [`Document<Text>`]: crate::document::Document
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[non_exhaustive]
pub enum TextExtraction {
    /// Structural parse of a text-bearing format: PDF text layer,
    /// DOCX XML runs, HTML, plain UTF-8.
    Native,
    /// Text obtained by OCR'ing an image-backed page (image-only PDF,
    /// scanned document).
    Recognized(ModelProvenance),
}

/// How a [`Document<Image>`]'s content was produced.
///
/// Every image-modality document is the output of *some* recognition
/// pass over pixels; the variant names which pass ran. [`Pending`]
/// is the importer-time placeholder before any extractor has run;
/// the extractor stage replaces it with the concrete variant
/// carrying the backend's [`ModelProvenance`].
///
/// [`Document<Image>`]: crate::document::Document
/// [`Pending`]: Self::Pending
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[non_exhaustive]
pub enum ImageExtraction {
    /// No extractor has run yet. Importer stamps this; the
    /// extractor stage replaces it once an OCR / scene-text /
    /// object-detection / layout backend produces blocks.
    Pending,
    /// Optical character recognition: raster text (printed or
    /// handwritten) converted into machine-readable characters.
    Ocr(ModelProvenance),
    /// Scene text detection: text embedded in natural images (signs,
    /// screens, whiteboards) localised prior to OCR.
    SceneText(ModelProvenance),
    /// Object detection: regions of interest located and labelled
    /// within an image or video frame.
    ObjectDetection(ModelProvenance),
    /// Document layout analysis: structural regions (headers,
    /// footers, signature blocks, form fields) identified by spatial
    /// arrangement rather than content.
    LayoutAnalysis(ModelProvenance),
}

/// How a [`Document<Audio>`]'s content was produced.
///
/// [`Pending`] is the importer-time placeholder before any extractor
/// has run; the extractor stage replaces it with the concrete
/// variant carrying the backend's [`ModelProvenance`].
///
/// [`Document<Audio>`]: crate::document::Document
/// [`Pending`]: Self::Pending
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[non_exhaustive]
pub enum AudioExtraction {
    /// No extractor has run yet. Importer stamps this; the STT or
    /// diarization backend replaces it once samples are processed.
    Pending,
    /// Speech-to-text transcription: audio samples converted into
    /// text segments.
    Transcription(ModelProvenance),
    /// Speaker diarization: audio segmented by speaker identity
    /// before recognition attributes utterances.
    Diarization(ModelProvenance),
}

/// How a [`Document<Tabular>`]'s structure was produced.
///
/// [`Document<Tabular>`]: crate::document::Document
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[non_exhaustive]
pub enum TabularExtraction {
    /// Header names and column types known from the source format
    /// (Parquet, Avro, CSV with a header row).
    SchemaTyped,
    /// Column semantics inferred from column data (header-less CSV,
    /// positional conventions).
    SchemaInferred,
    /// Tabular structure recovered from an image (row/column
    /// reconstruction from a scanned table); preserves cell
    /// relationships that plain OCR loses.
    Recovered(ModelProvenance),
}

impl TabularExtraction {
    /// Map the codec's `has_header()` signal to the matching
    /// extraction variant.
    ///
    /// `None` means the codec couldn't decide (non-tabular handle
    /// somehow reached this site) — fall back to `SchemaInferred`
    /// rather than panicking, since over-reporting "inferred" is
    /// safe.
    pub fn from_header_signal(has_header: Option<bool>) -> Self {
        match has_header {
            Some(true) => Self::SchemaTyped,
            Some(false) | None => Self::SchemaInferred,
        }
    }
}
