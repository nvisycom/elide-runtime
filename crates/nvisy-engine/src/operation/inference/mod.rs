//! Inference operations: ML/AI model calls that extract structured information.

mod classification;
mod computer_vision;
mod ner;
mod ocr;
mod summarization;
mod transcription;
mod translation;

#[allow(unused_imports)]
pub use classification::Classification;
#[allow(unused_imports)]
pub use computer_vision::ComputerVision;
#[allow(unused_imports)]
pub use ner::Ner;
#[allow(unused_imports)]
pub use ocr::Ocr;
#[allow(unused_imports)]
pub use summarization::Summarization;
#[allow(unused_imports)]
pub use transcription::Transcription;
#[allow(unused_imports)]
pub use translation::Translation;
