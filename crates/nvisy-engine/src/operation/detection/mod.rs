//! Entity recognition operations: NER (language-model) and pattern
//! matching.
//!
//! Both methods detect entities in extracted text and run sequentially
//! within the detection phase. They share the [`RebaseEntities`]
//! extension trait for shifting per-span offsets onto document
//! coordinates.

mod entity_recognition;
mod pattern_engine;
mod pattern_recognition;
mod rebase_entities;

pub(crate) use self::entity_recognition::EntityRecognition;
pub(crate) use self::pattern_recognition::PatternRecognition;
