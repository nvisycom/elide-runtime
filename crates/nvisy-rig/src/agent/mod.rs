//! Specialized detection agents: NER (text), CV (vision), and OCR (image-to-text).
//!
//! Each agent composes a [`BaseAgent`](base::BaseAgent) with domain-specific
//! prompts and optional tools. Public types are re-exported from [`crate`] —
//! consumer code should not reach into submodules.

mod base;
mod cv;
mod ner;
mod ocr;

pub use base::BaseAgentConfig;
pub(crate) use base::BaseAgent;

pub use cv::{CvAgent, CvDetection, CvEntities, CvEntity, CvProvider};
pub use ner::{NerAgent, NerEntities, NerEntity};
pub use ocr::{OcrAgent, OcrEntity, OcrOutput, OcrProvider, OcrTextRegion};
