//! Inference operations: ML/AI model calls that extract structured information.
//!
//! These operations delegate to external agents or engines (LLMs, OCR, NER,
//! computer vision) and return domain objects such as [`Entity`] or OCR
//! [`ImageOutput`].
//!
//! | Operation             | Description                                        |
//! |-----------------------|----------------------------------------------------|
//! | [`Ocr`]               | Extracts text regions from images via OCR engine   |
//! | [`OcrVerification`]   | Confirms/rejects OCR entities with a VLM           |
//! | [`Ner`]               | Named-entity recognition with coreference state    |
//! | [`ComputerVision`]    | Object detection + classification on images        |
//!
//! Audio transcription is handled by [`SttService`] within the
//! `AudialExtraction` handler, not as a standalone operation.
//! Summarization and translation are steps within [`GenerateContext`].
//!
//! [`Entity`]: nvisy_ontology::entity::Entity
//! [`ImageOutput`]: nvisy_ocr::ImageOutput
//! [`SttService`]: nvisy_rig::audio::stt::SttService
//! [`GenerateContext`]: crate::operation::lifecycle::GenerateContext

mod computer_vision;
mod ner;
mod ocr;
mod ocr_verification;

pub use self::computer_vision::ComputerVision;
pub use self::ner::{Ner, NerMethodParams};
pub use self::ocr::Ocr;
pub use self::ocr_verification::{OcrVerification, OcrVerificationInput};
