//! Inference operations: ML/AI model calls that extract structured information.

mod classification;
mod computer_vision;
mod ner;
mod ocr;
mod ocr_verification;
mod summarization;
mod transcription;
mod translation;

pub use classification::Classification;
pub use computer_vision::ComputerVision;
pub use ner::{Ner, NerMethodParams};
pub use ocr::Ocr;
pub use ocr_verification::{OcrVerification, OcrVerificationInput};
pub use summarization::Summarization;
pub use transcription::Transcription;
pub use translation::Translation;
