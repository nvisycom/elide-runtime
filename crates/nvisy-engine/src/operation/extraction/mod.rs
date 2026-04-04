//! Extraction operations: visual (OCR), audial (STT), and text.
//!
//! Each modality extracts structured text from raw content, preparing
//! it for downstream detection phases.

mod speech;
mod vision;

pub(crate) use self::speech::AudialExtractionOp;
pub(crate) use self::vision::VisualExtractionOp;
