//! Agent system: base agent, specialized agents, and tool-provider traits.
//!
//! All public types are re-exported here — consumer code should not reach
//! into individual agent submodules.

pub(crate) mod base;
mod cv;
mod ocr;
mod ner;

pub(crate) use base::BaseAgent;
pub use base::{AuthenticatedProvider, BaseAgentConfig, ContextWindow, Provider, UnauthenticatedProvider};

pub use ner::{NerAgent, RawEntities, RawEntity};
pub use ocr::{OcrAgent, OcrOutput, OcrProvider, OcrTextRegion, RawOcrEntity};
pub use cv::{CvAgent, CvDetection, CvProvider, RawCvEntities, RawCvEntity};
