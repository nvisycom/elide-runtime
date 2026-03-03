//! Specialized detection agents: NER (text), CV (vision), OCR (image-to-text),
//! and text generation (synthetic replacement values).
//!
//! Each agent composes a [`BaseAgent`](base::BaseAgent) with domain-specific
//! prompts and optional tools. Public types are re-exported from [`crate`] —
//! consumer code should not reach into submodules.

mod base;
mod cv;
mod generate;
mod ner;
mod ocr;

pub use base::{AgentProvider, AgentConfig, ContextWindow, DetectionConfig, DetectionRequest, DetectionResponse};
pub(crate) use base::{BaseAgent, ALL_TYPES_HINT};

pub use cv::{CvAgent, CvDetection, CvEntities, CvEntity, CvProvider};
pub use generate::{GenAgent, GenOutput, GenRequest, GeneratedEntity};
pub use ner::{KnownNerEntity, NerAgent, NerContext, NerEntities, NerEntity, ResolvedOffsets};
pub use ocr::{OcrAgent, OcrEntity, OcrOutput, OcrProvider, OcrTextRegion};
