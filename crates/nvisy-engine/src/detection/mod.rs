//! Detection: [`Recognizer`] trait, [`DetectionEngine`] orchestrator,
//! built-in recognizer adapters (NER, pattern, LLM).
//!
//! This module hosts the recognizer-side machinery the engine
//! consumes. The trait itself ([`Recognizer`], with an associated
//! `Context` type) lives in `nvisy-core` so that backend crates
//! could in principle implement it without depending on the engine.
//! Today every built-in adapter lives here, alongside the engine
//! that runs them.

mod engine;
mod extension;
mod recognizer;

pub use nvisy_core::detection::{DetectionParams, Recognizer};
pub use nvisy_pattern::PatternFilter;

pub use self::engine::{
    Detection, DetectionContext, DetectionContextBuilder, DetectionContextBuilderError,
    DetectionEngine, DetectionEngineBuilder, DetectionEngineBuilderError, DynRecognizer,
};
pub use self::extension::RebaseEntities;
pub use self::recognizer::{
    LlmContext, LlmDetection, LlmRecognizer, NerContext, NerDetection, NerRecognizer,
    PatternContext, PatternDetection, PatternRecognizer,
};
