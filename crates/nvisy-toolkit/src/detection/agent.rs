//! Re-export the [`nvisy_agent`] surface as
//! `nvisy_toolkit::detection::agent`.
//!
//! A consumer that wants the shipped LLM-driven recognizer
//! ([`LlmRecognizer<M>`], implementing [`EntityRecognizer<M>`]) only
//! needs the `nvisy-toolkit` dep. The recognizer registers straight
//! into the shared [`RecognizerRegistry`] like any other recognizer.
//!
//! [`LlmRecognizer<M>`]: nvisy_agent::LlmRecognizer
//! [`EntityRecognizer<M>`]: nvisy_core::EntityRecognizer
//! [`RecognizerRegistry`]: super::RecognizerRegistry

pub use nvisy_agent::*;
