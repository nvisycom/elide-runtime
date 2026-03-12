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
//! | [`Classification`]    | Content sensitivity/topic classification           |
//! | [`Summarization`]     | Summarizes text content                            |
//! | [`Transcription`]     | Audio-to-text transcription                        |
//! | [`Translation`]       | Cross-language text translation                    |
//!
//! [`Entity`]: nvisy_ontology::entity::Entity
//! [`ImageOutput`]: nvisy_ocr::ImageOutput

mod classification;
mod computer_vision;
mod ner;
mod ocr;
mod ocr_verification;
mod summarization;
mod transcription;
mod translation;

pub use self::classification::Classification;
pub use self::computer_vision::ComputerVision;
pub use self::ner::{Ner, NerMethodParams};
pub use self::ocr::Ocr;
pub use self::ocr_verification::{OcrVerification, OcrVerificationInput};
pub use self::summarization::Summarization;
pub use self::transcription::Transcription;
pub use self::translation::Translation;
