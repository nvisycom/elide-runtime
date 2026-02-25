//! Agent system: base agent, specialized agents, and tool-provider traits.
//!
//! All public types are re-exported here — consumer code should not reach
//! into individual agent submodules.

pub(crate) mod base;
mod detect;
mod extract;
mod recognize;

pub(crate) use base::BaseAgent;
pub use base::{AuthenticatedProvider, BaseAgentConfig, ContextWindow, Provider, UnauthenticatedProvider};

pub use recognize::{NerAgent, RawEntities, RawEntity};
pub use extract::{OcrAgent, OcrOutput, OcrProvider, OcrTextRegion, RawOcrEntity};
pub use detect::{CvAgent, CvDetection, CvProvider, RawCvEntities, RawCvEntity};
