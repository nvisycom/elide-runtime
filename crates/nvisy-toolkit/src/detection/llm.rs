//! Re-export the [`nvisy_llm`] surface as
//! `nvisy_toolkit::detection::llm`.
//!
//! A consumer that wants the shipped LLM-driven recognizer
//! ([`LlmRecognizer<M>`], implementing [`EntityRecognizer<M>`]) only
//! needs the `nvisy-toolkit` dep. The recognizer registers straight
//! into the shared [`RecognizerRegistry`] like any other recognizer.
//!
//! [`LlmRecognizer<M>`]: nvisy_llm::LlmRecognizer
//! [`EntityRecognizer<M>`]: nvisy_core::recognition::EntityRecognizer
//! [`RecognizerRegistry`]: super::RecognizerRegistry

pub use nvisy_llm::*;
