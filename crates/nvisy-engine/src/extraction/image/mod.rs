//! Image-modality extraction.
//!
//! Today's only image extraction technique is OCR ([`ocr`]). Future
//! techniques (e.g. layout segmentation, scene-text detection) would
//! live as sibling sub-modules and stack inside the image arm of
//! [`ExtractionPhase::apply`].
//!
//! [`ExtractionPhase::apply`]: super::ExtractionPhase::apply

#[cfg(feature = "image")]
pub mod ocr;

#[cfg(feature = "image")]
pub use self::ocr::{OcrExtractor, OcrExtractorConfig};
