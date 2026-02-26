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

pub use ner::{NerAgent, NerEntities, NerEntity};
pub use ocr::{OcrAgent, OcrEntity, OcrOutput, OcrProvider, OcrTextRegion};
pub use cv::{CvAgent, CvDetection, CvEntities, CvEntity, CvProvider};
