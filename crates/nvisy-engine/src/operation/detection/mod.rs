//! Entity recognition operations: LLM-driven NER, trait-backed NER
//! over [`nvisy_nlp::NerBackend`], and pattern matching.
//!
//! All three methods detect entities in extracted text and run
//! sequentially within the detection phase. They share the
//! [`RebaseEntities`] extension trait for shifting per-span offsets
//! onto document coordinates.

mod llm_recognition;
mod ner_recognition;
mod pattern_engine;
mod pattern_recognition;
mod rebase_entities;

pub(crate) use self::llm_recognition::LlmRecognition;
pub(crate) use self::ner_recognition::NerRecognition;
pub(crate) use self::pattern_recognition::PatternRecognition;
