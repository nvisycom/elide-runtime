//! Detection operations: NER (language model) and pattern matching.
//!
//! Both methods detect entities in extracted text. They are logically
//! independent and run sequentially per document within the detection
//! phase.

mod entity_recognition;
mod pattern_recognition;

pub(crate) use self::entity_recognition::EntityRecognitionOp;
pub(crate) use self::pattern_recognition::PatternRecognitionOp;
