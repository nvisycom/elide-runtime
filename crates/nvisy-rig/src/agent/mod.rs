//! Agent system: base agent, specialized agents, and tool-provider traits.
//!
//! All public types are re-exported here — consumer code should not reach
//! into individual agent submodules.

mod base;
mod context;
mod detect;
mod extract;
mod recognize;
mod redactor;

pub(crate) use base::{BaseAgent, BaseAgentBuilder, BaseAgentConfig};
pub(crate) use context::ContextWindow;

pub use recognize::{NerAgent, RawEntities, RawEntity};
pub use extract::{OcrAgent, OcrOutput, OcrProvider, RawOcrEntity};
pub use detect::{CvAgent, CvDetection, CvProvider, RawCvEntities, RawCvEntity};
pub use redactor::{RawRedaction, RedactorAgent, RedactorOutput};
