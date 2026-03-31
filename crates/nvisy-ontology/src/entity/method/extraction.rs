//! Content extraction method classification.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumString};

/// How content was extracted from its source modality into analyzable form.
///
/// Each variant names the technique that transformed raw content
/// (image pixels, audio samples, binary file formats) into a
/// representation suitable for entity recognition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[derive(Display, EnumString, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
#[non_exhaustive]
pub enum ExtractionMethod {
    /// Structural parsing of document formats (PDF, DOCX, HTML)
    /// into text and layout primitives.
    DocumentParsing,
    /// Inference of field semantics from column names, types, or
    /// positional conventions in tabular data.
    SchemaInference,
    /// Optical character recognition: converts raster text
    /// (printed or handwritten) into machine-readable characters.
    OpticalCharacterRecognition,
    /// Object detection: locates and labels regions of interest
    /// within an image or video frame.
    ObjectDetection,
    /// Scene text detection: localises text embedded in natural
    /// images (signs, screens, whiteboards) prior to OCR.
    SceneTextDetection,
    /// Table extraction: recovers row/column structure from images
    /// or scanned PDFs, preserving cell relationships that plain
    /// OCR loses.
    TableExtraction,
    /// Document layout analysis: identifies structural regions
    /// (headers, footers, signature blocks, form fields) by spatial
    /// arrangement rather than content.
    LayoutAnalysis,
    /// Metadata extraction: reads EXIF, PDF properties, or other
    /// embedded metadata that may contain PII (author, GPS, device info).
    MetadataExtraction,
    /// Frame extraction: samples individual frames from video
    /// streams for downstream image analysis.
    FrameExtraction,
    /// Speech-to-text transcription: converts audio into text.
    Transcription,
    /// Speaker diarization: segments audio by speaker identity
    /// to attribute utterances before recognition.
    Diarization,
}
