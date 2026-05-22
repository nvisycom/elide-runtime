//! Built-in [`Recognizer`] implementations.
//!
//! Each recognizer impl lives in its own submodule with its
//! typed per-call context:
//!
//! - [`NerRecognizer`] wraps `nvisy_nlp::Engine` (NER + optional
//!   language detection, tokens, keywords). Consumes [`NerContext`].
//! - [`PatternRecognizer`] wraps `nvisy_pattern::PatternEngine`
//!   (regex, dictionary, allow/deny, context-aware boosting).
//!   Consumes [`PatternContext`].
//! - [`LlmRecognizer`] wraps `nvisy_rig::pipeline::NerPipeline`
//!   (LLM-driven detection with coreference state). Consumes
//!   [`LlmContext`].
//!
//! The [`Recognizer`] trait itself lives in `nvisy_core::detection`
//! and is associated-type based — each impl declares its own
//! `Context`. A [`DynRecognizer`] bridge in [`crate::engine`]
//! erases the associated type so the engine can hold a
//! heterogeneous collection of recognizers.
//!
//! [`Recognizer`]: nvisy_core::detection::Recognizer
//! [`DynRecognizer`]: crate::engine::DynRecognizer

mod language_model;
mod named_entity;
mod pattern;

pub use self::language_model::{LlmContext, LlmDetection, LlmRecognizer};
pub use self::named_entity::{NerContext, NerDetection, NerRecognizer};
pub use self::pattern::{PatternContext, PatternDetection, PatternRecognizer};
