//! Re-export the [`nvisy_ocr`] backend surface as
//! `nvisy_toolkit::extraction::ocr`.
//!
//! A consumer that wants the shipped OCR backends only needs the
//! `nvisy-toolkit` dep — `nvisy_toolkit::extraction::ocr::OcrExtractor`,
//! `nvisy_toolkit::extraction::ocr::backend::OcrBackend`,
//! `nvisy_toolkit::extraction::ocr::backend::NoopBackend`, etc. are
//! all reachable here.

use nvisy_ocr::backend::OcrResponse;
pub use nvisy_ocr::*;

/// Output shape produced by every image-modality extractor.
///
/// Pinned to [`OcrResponse`] today. Widening the type later (to
/// accommodate scene-text / layout-detection backends with
/// different shapes) is a breaking change worth taking deliberately
/// rather than hiding behind generics.
///
/// [`OcrResponse`]: nvisy_ocr::backend::OcrResponse
pub type ImageExtractorOutput = OcrResponse;
