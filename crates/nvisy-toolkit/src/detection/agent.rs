//! Re-export the [`nvisy_agent`] surface as
//! `nvisy_toolkit::detection::agent`.
//!
//! A consumer that wants the shipped LLM-driven recognizers — the
//! text-side [`NerAgent`] (impls
//! [`EntityRecognizer<Text>`](nvisy_core::EntityRecognizer)) and the
//! image-side [`VlmAgent`] (impls
//! [`EntityRecognizer<Image>`](nvisy_core::EntityRecognizer)) — only
//! needs the `nvisy-toolkit` dep. Both register straight into the
//! shared [`RecognizerRegistry`](super::RecognizerRegistry) like any
//! other recognizer.
//!
//! [`NerAgent`]: nvisy_agent::agent::ner::NerAgent
//! [`VlmAgent`]: nvisy_agent::agent::vlm::VlmAgent

pub use nvisy_agent::*;
